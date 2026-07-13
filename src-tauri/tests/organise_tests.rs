use sonoscope_lib::organise::{self, OrganiseMode};
use std::fs;
use std::path::Path;
use tempfile::TempDir;

async fn make_library(dir: &TempDir) -> sqlx::SqlitePool {
    let db_path = dir.path().join("library.db");
    let pool = sonoscope_lib::db::open_pool(&db_path).await.expect("open pool");
    sonoscope_lib::library::open::open_or_create_library(dir.path().to_str().unwrap(), &pool)
        .await
        .expect("open library");
    pool
}

async fn discover(dir: &TempDir, pool: &sqlx::SqlitePool) {
    sonoscope_lib::library::discover::run_discovery(dir.path(), pool, |_| {})
        .await
        .expect("discovery");
}

async fn sample_id_by_filename(pool: &sqlx::SqlitePool, filename: &str) -> i64 {
    let (id,): (i64,) = sqlx::query_as("SELECT id FROM samples WHERE filename = ?")
        .bind(filename)
        .fetch_one(pool)
        .await
        .unwrap();
    id
}

async fn tag_primary(pool: &sqlx::SqlitePool, sample_id: i64, dimension: &str, value: &str) {
    sonoscope_lib::commands::write_user_tag(pool, sample_id, dimension, value)
        .await
        .unwrap();
}

async fn sample_paths(pool: &sqlx::SqlitePool, sample_id: i64) -> (String, String) {
    sqlx::query_as("SELECT path, relative_path FROM samples WHERE id = ?")
        .bind(sample_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

fn write_sample(dir: &Path, name: &str) {
    fs::write(dir.join(name), b"fake").unwrap();
}

#[tokio::test]
async fn test_preview_resolves_tagged_and_untagged_samples() {
    let dir = TempDir::new().unwrap();
    write_sample(dir.path(), "kick.wav");
    write_sample(dir.path(), "mystery.wav");
    let pool = make_library(&dir).await;
    discover(&dir, &pool).await;

    let kick_id = sample_id_by_filename(&pool, "kick.wav").await;
    tag_primary(&pool, kick_id, "Type", "one-shot").await;
    tag_primary(&pool, kick_id, "Instrument", "kick").await;

    let preview = organise::preview_organise(&pool, "{Type}/{Instrument}", None)
        .await
        .unwrap();

    assert_eq!(preview.total, 2);
    assert_eq!(preview.untagged_count, 1);
    assert_eq!(preview.conflict_count, 0);

    let kick = preview
        .entries
        .iter()
        .find(|entry| entry.sample_id == kick_id)
        .unwrap();
    assert!(!kick.untagged);
    assert_eq!(
        kick.to.replace('\\', "/"),
        "one-shot/kick/kick.wav".to_string()
    );

    let mystery = preview
        .entries
        .iter()
        .find(|entry| entry.sample_id != kick_id)
        .unwrap();
    assert!(mystery.untagged);
    assert_eq!(
        mystery.to.replace('\\', "/"),
        "_untagged/mystery.wav".to_string()
    );
}

#[tokio::test]
async fn test_preview_rejects_unknown_dimension() {
    let dir = TempDir::new().unwrap();
    let pool = make_library(&dir).await;

    let result = organise::preview_organise(&pool, "{Nonsense}", None).await;

    assert!(matches!(
        result,
        Err(sonoscope_lib::error::CommandError::Other(message))
            if message.contains("Unknown dimension")
    ));
}

#[tokio::test]
async fn test_preview_marks_in_plan_target_collisions() {
    let dir = TempDir::new().unwrap();
    write_sample(dir.path(), "clash.wav");
    let sub = dir.path().join("nested");
    fs::create_dir(&sub).unwrap();
    write_sample(&sub, "clash.wav");
    let pool = make_library(&dir).await;
    discover(&dir, &pool).await;

    // Both files are untagged, so both resolve to _untagged/clash.wav.
    let preview = organise::preview_organise(&pool, "{Type}", None).await.unwrap();

    assert_eq!(preview.total, 2);
    assert_eq!(preview.conflict_count, 1);
}

#[tokio::test]
async fn test_apply_move_relocates_files_and_updates_database() {
    let dir = TempDir::new().unwrap();
    write_sample(dir.path(), "kick.wav");
    write_sample(dir.path(), "mystery.wav");
    let pool = make_library(&dir).await;
    discover(&dir, &pool).await;

    let kick_id = sample_id_by_filename(&pool, "kick.wav").await;
    tag_primary(&pool, kick_id, "Type", "one-shot").await;
    tag_primary(&pool, kick_id, "Instrument", "kick").await;

    let result = organise::apply_organise(
        &pool,
        dir.path(),
        "{Type}/{Instrument}",
        OrganiseMode::Move,
        None,
        None,
        |_, _| {},
    )
    .await
    .unwrap();

    assert_eq!(result.processed, 2);
    assert_eq!(result.skipped, 0);
    assert!(result.errors.is_empty());
    assert!(result.batch_id.is_some());

    let moved = dir.path().join("one-shot").join("kick").join("kick.wav");
    let untagged = dir.path().join("_untagged").join("mystery.wav");
    assert!(moved.is_file());
    assert!(untagged.is_file());
    assert!(!dir.path().join("kick.wav").exists());

    let (path, relative_path) = sample_paths(&pool, kick_id).await;
    assert_eq!(path, moved.to_string_lossy());
    assert_eq!(
        relative_path.replace('\\', "/"),
        "one-shot/kick/kick.wav".to_string()
    );

    let (op_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM file_operations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(op_count, 2);

    let batches = organise::list_batches(&pool).await.unwrap();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].file_count, 2);
    assert_eq!(batches[0].mode, OrganiseMode::Move);
    assert_eq!(batches[0].status, organise::BatchStatus::Completed);
}

#[tokio::test]
async fn test_apply_move_skips_files_already_at_target() {
    let dir = TempDir::new().unwrap();
    write_sample(dir.path(), "kick.wav");
    let pool = make_library(&dir).await;
    discover(&dir, &pool).await;

    let kick_id = sample_id_by_filename(&pool, "kick.wav").await;
    tag_primary(&pool, kick_id, "Type", "one-shot").await;

    let first = organise::apply_organise(
        &pool,
        dir.path(),
        "{Type}",
        OrganiseMode::Move,
        None,
        None,
        |_, _| {},
    )
    .await
    .unwrap();
    assert_eq!(first.processed, 1);

    // Re-applying the same pattern must be a no-op and create no batch.
    let second = organise::apply_organise(
        &pool,
        dir.path(),
        "{Type}",
        OrganiseMode::Move,
        None,
        None,
        |_, _| {},
    )
    .await
    .unwrap();

    assert_eq!(second.processed, 0);
    assert_eq!(second.skipped, 1);
    assert!(second.batch_id.is_none());
    assert_eq!(organise::list_batches(&pool).await.unwrap().len(), 1);
}

#[tokio::test]
async fn test_apply_respects_sample_id_filter() {
    let dir = TempDir::new().unwrap();
    write_sample(dir.path(), "kick.wav");
    write_sample(dir.path(), "snare.wav");
    let pool = make_library(&dir).await;
    discover(&dir, &pool).await;

    let kick_id = sample_id_by_filename(&pool, "kick.wav").await;
    tag_primary(&pool, kick_id, "Type", "one-shot").await;

    let result = organise::apply_organise(
        &pool,
        dir.path(),
        "{Type}",
        OrganiseMode::Move,
        None,
        Some(&[kick_id]),
        |_, _| {},
    )
    .await
    .unwrap();

    assert_eq!(result.total, 1);
    assert_eq!(result.processed, 1);
    assert!(dir.path().join("snare.wav").is_file(), "unselected file must not move");
}

#[tokio::test]
async fn test_apply_copy_duplicates_files_without_touching_library() {
    let dir = TempDir::new().unwrap();
    let destination = TempDir::new().unwrap();
    write_sample(dir.path(), "kick.wav");
    let pool = make_library(&dir).await;
    discover(&dir, &pool).await;

    let kick_id = sample_id_by_filename(&pool, "kick.wav").await;
    tag_primary(&pool, kick_id, "Type", "one-shot").await;
    let (path_before, relative_before) = sample_paths(&pool, kick_id).await;

    let result = organise::apply_organise(
        &pool,
        dir.path(),
        "{Type}",
        OrganiseMode::Copy,
        Some(destination.path()),
        None,
        |_, _| {},
    )
    .await
    .unwrap();

    assert_eq!(result.processed, 1);
    assert!(dir.path().join("kick.wav").is_file(), "source must remain");
    assert!(destination
        .path()
        .join("one-shot")
        .join("kick.wav")
        .is_file());

    let (path_after, relative_after) = sample_paths(&pool, kick_id).await;
    assert_eq!(path_before, path_after);
    assert_eq!(relative_before, relative_after);

    let batches = organise::list_batches(&pool).await.unwrap();
    assert_eq!(batches[0].mode, OrganiseMode::Copy);
}

#[tokio::test]
async fn test_apply_copy_requires_destination_outside_library() {
    let dir = TempDir::new().unwrap();
    write_sample(dir.path(), "kick.wav");
    let pool = make_library(&dir).await;
    discover(&dir, &pool).await;

    let inside = dir.path().join("export");
    fs::create_dir(&inside).unwrap();

    let result = organise::apply_organise(
        &pool,
        dir.path(),
        "{Type}",
        OrganiseMode::Copy,
        Some(&inside),
        None,
        |_, _| {},
    )
    .await;

    assert!(matches!(
        result,
        Err(sonoscope_lib::error::CommandError::Other(message))
            if message.contains("outside the library")
    ));
}

#[tokio::test]
async fn test_apply_skips_when_target_exists_on_disk() {
    let dir = TempDir::new().unwrap();
    write_sample(dir.path(), "kick.wav");
    let occupied = dir.path().join("one-shot");
    fs::create_dir(&occupied).unwrap();
    write_sample(&occupied, "kick.wav");
    let pool = make_library(&dir).await;
    discover(&dir, &pool).await;

    // Only organise the root file; the occupying file keeps its place.
    let root_kick = {
        let (id,): (i64,) =
            sqlx::query_as("SELECT id FROM samples WHERE relative_path = 'kick.wav'")
                .fetch_one(&pool)
                .await
                .unwrap();
        id
    };
    tag_primary(&pool, root_kick, "Type", "one-shot").await;

    let result = organise::apply_organise(
        &pool,
        dir.path(),
        "{Type}",
        OrganiseMode::Move,
        None,
        Some(&[root_kick]),
        |_, _| {},
    )
    .await
    .unwrap();

    assert_eq!(result.processed, 0);
    assert_eq!(result.skipped, 1);
    assert!(dir.path().join("kick.wav").is_file(), "skipped file must not move");
}

#[tokio::test]
async fn test_rollback_restores_moved_files_and_paths() {
    let dir = TempDir::new().unwrap();
    write_sample(dir.path(), "kick.wav");
    let loops = dir.path().join("loops");
    fs::create_dir(&loops).unwrap();
    write_sample(&loops, "groove.wav");
    let pool = make_library(&dir).await;
    discover(&dir, &pool).await;

    let kick_id = sample_id_by_filename(&pool, "kick.wav").await;
    let groove_id = sample_id_by_filename(&pool, "groove.wav").await;
    tag_primary(&pool, kick_id, "Type", "one-shot").await;
    tag_primary(&pool, groove_id, "Type", "loop").await;

    let apply = organise::apply_organise(
        &pool,
        dir.path(),
        "Sorted/{Type}",
        OrganiseMode::Move,
        None,
        None,
        |_, _| {},
    )
    .await
    .unwrap();
    let batch_id = apply.batch_id.unwrap();
    assert!(!dir.path().join("kick.wav").exists());

    let rollback = organise::rollback_batch(&pool, batch_id).await.unwrap();

    assert_eq!(rollback.restored, 2);
    assert_eq!(rollback.skipped, 0);
    assert!(dir.path().join("kick.wav").is_file());
    assert!(loops.join("groove.wav").is_file());

    let (path, relative_path) = sample_paths(&pool, kick_id).await;
    assert_eq!(path, dir.path().join("kick.wav").to_string_lossy());
    assert_eq!(relative_path, "kick.wav");

    let batches = organise::list_batches(&pool).await.unwrap();
    assert_eq!(batches[0].status, organise::BatchStatus::RolledBack);

    let result = organise::rollback_batch(&pool, batch_id).await;
    assert!(
        matches!(
            result,
            Err(sonoscope_lib::error::CommandError::Other(message))
                if message.contains("already rolled back")
        ),
        "second rollback must be rejected"
    );
}

#[tokio::test]
async fn test_rollback_rejects_copy_batches() {
    let dir = TempDir::new().unwrap();
    let destination = TempDir::new().unwrap();
    write_sample(dir.path(), "kick.wav");
    let pool = make_library(&dir).await;
    discover(&dir, &pool).await;

    let apply = organise::apply_organise(
        &pool,
        dir.path(),
        "{Type}",
        OrganiseMode::Copy,
        Some(destination.path()),
        None,
        |_, _| {},
    )
    .await
    .unwrap();

    let result = organise::rollback_batch(&pool, apply.batch_id.unwrap()).await;

    assert!(matches!(
        result,
        Err(sonoscope_lib::error::CommandError::Other(message))
            if message.contains("Only move batches")
    ));
}

#[tokio::test]
async fn test_presets_seeded_saved_and_deleted() {
    let dir = TempDir::new().unwrap();
    let pool = make_library(&dir).await;

    let presets = organise::list_presets(&pool).await.unwrap();
    let names: Vec<&str> = presets.iter().map(|preset| preset.name.as_str()).collect();
    assert!(names.contains(&"Type / Instrument"));
    assert!(names.contains(&"Instrument / Type"));
    assert!(presets.iter().all(|preset| preset.is_system));

    let saved = organise::save_preset(&pool, "By key", "{Key}/{Type}")
        .await
        .unwrap();
    assert!(!saved.is_system);
    assert_eq!(saved.pattern, "{Key}/{Type}");

    // Saving the same name again updates the pattern in place.
    let updated = organise::save_preset(&pool, "By key", "{Key}")
        .await
        .unwrap();
    assert_eq!(updated.id, saved.id);
    assert_eq!(updated.pattern, "{Key}");

    assert!(organise::save_preset(&pool, "  ", "{Key}").await.is_err());
    assert!(organise::save_preset(&pool, "Broken", "{Key").await.is_err());

    organise::delete_preset(&pool, saved.id).await.unwrap();
    let after_delete = organise::list_presets(&pool).await.unwrap();
    assert!(after_delete.iter().all(|preset| preset.id != saved.id));
}

#[tokio::test]
async fn test_numeric_dimension_resolves_in_pattern() {
    let dir = TempDir::new().unwrap();
    write_sample(dir.path(), "groove.wav");
    let pool = make_library(&dir).await;
    discover(&dir, &pool).await;

    let groove_id = sample_id_by_filename(&pool, "groove.wav").await;
    tag_primary(&pool, groove_id, "Tempo", "120").await;

    let preview = organise::preview_organise(&pool, "{Tempo}bpm", None)
        .await
        .unwrap();

    assert_eq!(
        preview.entries[0].to.replace('\\', "/"),
        "120bpm/groove.wav".to_string()
    );
}
