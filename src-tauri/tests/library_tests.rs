use std::fs;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tempfile::TempDir;

async fn make_pool(dir: &TempDir) -> sqlx::SqlitePool {
    let db_path = dir.path().join("library.db");
    sonoscope_lib::db::open_pool(&db_path)
        .await
        .expect("open pool")
}

#[tokio::test]
async fn test_open_creates_schema() {
    let dir = TempDir::new().unwrap();
    let pool = make_pool(&dir).await;

    let meta =
        sonoscope_lib::library::open::open_or_create_library(dir.path().to_str().unwrap(), &pool)
            .await
            .unwrap();

    assert_eq!(meta.root_path, dir.path().to_str().unwrap());
    assert!(meta.created_at > 0);
    assert!(meta.last_discovered_at.is_none());

    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM samples")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_open_seeds_system_tag_dimensions() {
    let dir = TempDir::new().unwrap();
    let pool = make_pool(&dir).await;
    sonoscope_lib::library::open::open_or_create_library(dir.path().to_str().unwrap(), &pool)
        .await
        .unwrap();

    let dimensions: Vec<(String, String)> =
        sqlx::query_as("SELECT name, value_type FROM dimensions ORDER BY sort_order")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        dimensions,
        vec![
            ("Type".to_string(), "enum".to_string()),
            ("Instrument".to_string(), "multi_enum".to_string()),
            ("Key".to_string(), "enum".to_string()),
            ("Mode".to_string(), "enum".to_string()),
            ("Tempo".to_string(), "numeric".to_string()),
            ("Mood".to_string(), "multi_enum".to_string()),
        ]
    );

    let (instrument_values,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)
         FROM dimension_values dv
         JOIN dimensions d ON d.id = dv.dimension_id
         WHERE d.name = 'Instrument'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(instrument_values >= 13);
}

#[tokio::test]
async fn test_tag_dimensions_lists_seeded_values() {
    let dir = TempDir::new().unwrap();
    let pool = make_pool(&dir).await;
    sonoscope_lib::library::open::open_or_create_library(dir.path().to_str().unwrap(), &pool)
        .await
        .unwrap();

    let dimensions = sonoscope_lib::commands::tag_dimensions(&pool)
        .await
        .unwrap();

    let type_dimension = dimensions
        .iter()
        .find(|dimension| dimension.name == "Type")
        .expect("Type dimension should be present");
    assert_eq!(
        type_dimension.value_type,
        sonoscope_lib::error::DimensionValueType::Enum
    );
    assert!(type_dimension.values.contains(&"loop".to_string()));
    assert!(type_dimension.values.contains(&"one-shot".to_string()));
    assert!(!type_dimension.values.contains(&"top-loop".to_string()));

    let mode_dimension = dimensions
        .iter()
        .find(|dimension| dimension.name == "Mode")
        .expect("Mode dimension should be present");
    assert_eq!(
        mode_dimension.value_type,
        sonoscope_lib::error::DimensionValueType::Enum
    );
    assert!(mode_dimension.values.contains(&"major".to_string()));
    assert!(mode_dimension.values.contains(&"minor".to_string()));

    let instrument_dimension = dimensions
        .iter()
        .find(|dimension| dimension.name == "Instrument")
        .expect("Instrument dimension should be present");
    assert!(instrument_dimension.values.contains(&"tops".to_string()));
    assert!(instrument_dimension.values.contains(&"drums".to_string()));

    let tempo_dimension = dimensions
        .iter()
        .find(|dimension| dimension.name == "Tempo")
        .expect("Tempo dimension should be present");
    assert_eq!(
        tempo_dimension.value_type,
        sonoscope_lib::error::DimensionValueType::Numeric
    );
    assert!(tempo_dimension.values.is_empty());
}

#[tokio::test]
async fn test_open_is_idempotent() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("library.db");
    let root = dir.path().to_str().unwrap();

    let pool1 = sonoscope_lib::db::open_pool(&db_path).await.unwrap();
    let meta1 = sonoscope_lib::library::open::open_or_create_library(root, &pool1)
        .await
        .unwrap();

    let pool2 = sonoscope_lib::db::open_pool(&db_path).await.unwrap();
    let meta2 = sonoscope_lib::library::open::open_or_create_library(root, &pool2)
        .await
        .unwrap();

    assert_eq!(meta1.created_at, meta2.created_at);
}

#[tokio::test]
async fn test_discover_inserts_only_audio_files() {
    let dir = TempDir::new().unwrap();

    // Audio files that should be discovered
    for name in &["kick.wav", "snare.wav", "bass.mp3", "sub.flac"] {
        fs::write(dir.path().join(name), b"fake").unwrap();
    }
    // Non-audio file that must be ignored
    fs::write(dir.path().join("readme.txt"), b"not audio").unwrap();
    // Subdirectory with an audio file
    let sub = dir.path().join("loops");
    fs::create_dir(&sub).unwrap();
    fs::write(sub.join("groove.aiff"), b"fake").unwrap();

    let pool = make_pool(&dir).await;
    sonoscope_lib::library::open::open_or_create_library(dir.path().to_str().unwrap(), &pool)
        .await
        .unwrap();

    let count = sonoscope_lib::library::discover::run_discovery(dir.path(), &pool, |_| {})
        .await
        .unwrap();

    assert_eq!(count, 5, "should discover 5 audio files (not readme.txt)");

    let (db_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM samples")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(db_count, 5);
}

#[tokio::test]
async fn test_discover_is_idempotent() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("kick.wav"), b"fake").unwrap();

    let pool = make_pool(&dir).await;
    sonoscope_lib::library::open::open_or_create_library(dir.path().to_str().unwrap(), &pool)
        .await
        .unwrap();

    sonoscope_lib::library::discover::run_discovery(dir.path(), &pool, |_| {})
        .await
        .unwrap();
    sonoscope_lib::library::discover::run_discovery(dir.path(), &pool, |_| {})
        .await
        .unwrap();

    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM samples")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "re-scan must not create duplicate rows");
}

#[tokio::test]
async fn test_discover_cancellation_rolls_back_transaction() {
    let dir = TempDir::new().unwrap();
    for index in 0..75 {
        fs::write(dir.path().join(format!("sample-{index}.wav")), b"fake").unwrap();
    }

    let pool = make_pool(&dir).await;
    sonoscope_lib::library::open::open_or_create_library(dir.path().to_str().unwrap(), &pool)
        .await
        .unwrap();

    let cancellation = Arc::new(AtomicBool::new(false));
    let cancel_on_progress = cancellation.clone();
    let result = sonoscope_lib::library::discover::run_discovery_cancellable(
        dir.path(),
        &pool,
        cancellation,
        move |count| {
            if count >= 50 {
                cancel_on_progress.store(true, Ordering::Relaxed);
            }
        },
    )
    .await;

    assert!(
        matches!(
            result,
            Err(sonoscope_lib::error::CommandError::DiscoveryCancelled { count: 50 })
        ),
        "discovery should stop at the first cancellation check after progress"
    );

    let (sample_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM samples")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(sample_count, 0, "cancelled discovery must not commit rows");

    let (last_discovered_at,): (Option<i64>,) =
        sqlx::query_as("SELECT last_discovered_at FROM library_meta WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        last_discovered_at.is_none(),
        "cancelled discovery must not update library metadata"
    );
}

#[tokio::test]
async fn test_user_tag_write_and_clear_preserves_auto_tags() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("kick.wav"), b"fake").unwrap();

    let pool = make_pool(&dir).await;
    sonoscope_lib::library::open::open_or_create_library(dir.path().to_str().unwrap(), &pool)
        .await
        .unwrap();
    sonoscope_lib::library::discover::run_discovery(dir.path(), &pool, |_| {})
        .await
        .unwrap();

    let (sample_id,): (i64,) = sqlx::query_as("SELECT id FROM samples LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();

    let dimension_id = 2_i64;
    let (kick_value_id,): (i64,) =
        sqlx::query_as("SELECT id FROM dimension_values WHERE dimension_id = 2 AND value = 'kick'")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query(
        "INSERT INTO tags
         (sample_id, dimension_id, value_id, source, confidence, created_at)
         VALUES (?, ?, ?, 'heuristic', 0.9, 1)",
    )
    .bind(sample_id)
    .bind(dimension_id)
    .bind(kick_value_id)
    .execute(&pool)
    .await
    .unwrap();

    sonoscope_lib::commands::write_user_tag(&pool, sample_id, "Instrument", "snare")
        .await
        .unwrap();

    let (user_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM tags WHERE sample_id = ? AND source = 'user'")
            .bind(sample_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(user_count, 1);
    let (user_primary_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM tags WHERE sample_id = ? AND source = 'user' AND is_primary = 1",
    )
    .bind(sample_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let (auto_after_user_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM tags WHERE sample_id = ? AND source = 'heuristic'")
            .bind(sample_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let (auto_primary_while_user_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM tags WHERE sample_id = ? AND source = 'heuristic' AND is_primary = 1",
    )
    .bind(sample_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(user_primary_count, 1);
    assert_eq!(auto_after_user_count, 1);
    assert_eq!(auto_primary_while_user_count, 0);

    sonoscope_lib::commands::clear_user_tag_for_dimension(&pool, sample_id, dimension_id)
        .await
        .unwrap();

    let (auto_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM tags WHERE sample_id = ? AND source = 'heuristic'")
            .bind(sample_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let (remaining_user_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM tags WHERE sample_id = ? AND source = 'user'")
            .bind(sample_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(auto_count, 1);
    assert_eq!(remaining_user_count, 0);
    let (auto_primary_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM tags WHERE sample_id = ? AND source = 'heuristic' AND is_primary = 1",
    )
    .bind(sample_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(auto_primary_count, 1);
}

#[tokio::test]
async fn test_single_value_user_tag_replaces_existing_user_value() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("loop.wav"), b"fake").unwrap();

    let pool = make_pool(&dir).await;
    sonoscope_lib::library::open::open_or_create_library(dir.path().to_str().unwrap(), &pool)
        .await
        .unwrap();
    sonoscope_lib::library::discover::run_discovery(dir.path(), &pool, |_| {})
        .await
        .unwrap();

    let (sample_id,): (i64,) = sqlx::query_as("SELECT id FROM samples LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();

    sonoscope_lib::commands::write_user_tag(&pool, sample_id, "Type", "loop")
        .await
        .unwrap();
    sonoscope_lib::commands::write_user_tag(&pool, sample_id, "Type", "one-shot")
        .await
        .unwrap();

    let user_values: Vec<(String,)> = sqlx::query_as(
        "SELECT dv.value
         FROM tags t
         JOIN dimension_values dv ON dv.id = t.value_id
         WHERE t.sample_id = ? AND t.source = 'user'
         ORDER BY dv.value",
    )
    .bind(sample_id)
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(user_values, vec![("one-shot".to_string(),)]);
}

#[tokio::test]
async fn test_conflict_query_returns_unresolved_auto_tag_conflicts() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("loop.wav"), b"fake").unwrap();

    let pool = make_pool(&dir).await;
    sonoscope_lib::library::open::open_or_create_library(dir.path().to_str().unwrap(), &pool)
        .await
        .unwrap();
    sonoscope_lib::library::discover::run_discovery(dir.path(), &pool, |_| {})
        .await
        .unwrap();

    let (sample_id,): (i64,) = sqlx::query_as("SELECT id FROM samples LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let dimension_id = 1_i64;
    let (loop_value_id,): (i64,) =
        sqlx::query_as("SELECT id FROM dimension_values WHERE dimension_id = 1 AND value = 'loop'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let (one_shot_value_id,): (i64,) = sqlx::query_as(
        "SELECT id FROM dimension_values WHERE dimension_id = 1 AND value = 'one-shot'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO tags
         (sample_id, dimension_id, value_id, source, confidence, created_at)
         VALUES (?, ?, ?, 'heuristic', 0.8, 1), (?, ?, ?, 'model', 0.7, 1)",
    )
    .bind(sample_id)
    .bind(dimension_id)
    .bind(loop_value_id)
    .bind(sample_id)
    .bind(dimension_id)
    .bind(one_shot_value_id)
    .execute(&pool)
    .await
    .unwrap();

    let conflicts = sonoscope_lib::commands::conflicts_for_sample(&pool, sample_id)
        .await
        .unwrap();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].dimension, "Type");
    assert_eq!(conflicts[0].candidates.len(), 2);

    sonoscope_lib::commands::write_user_tag(&pool, sample_id, "Type", "loop")
        .await
        .unwrap();

    let resolved_conflicts = sonoscope_lib::commands::conflicts_for_sample(&pool, sample_id)
        .await
        .unwrap();
    assert!(
        resolved_conflicts.is_empty(),
        "user tag should resolve auto-tag conflicts for the dimension"
    );
}

#[tokio::test]
async fn test_requeue_all_samples_marks_existing_samples_pending() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("kick.wav"), b"fake").unwrap();

    let pool = make_pool(&dir).await;
    sonoscope_lib::library::open::open_or_create_library(dir.path().to_str().unwrap(), &pool)
        .await
        .unwrap();
    sonoscope_lib::library::discover::run_discovery(dir.path(), &pool, |_| {})
        .await
        .unwrap();

    sqlx::query("UPDATE samples SET analysis_status = 'done'")
        .execute(&pool)
        .await
        .unwrap();

    sonoscope_lib::analysis::requeue_all_samples(&pool)
        .await
        .unwrap();

    let (pending_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM samples WHERE analysis_status = 'pending'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(pending_count, 1);
}

#[tokio::test]
async fn test_playback_sample_returns_validated_sample_path() {
    let dir = TempDir::new().unwrap();
    let sample_path = dir.path().join("kick.wav");
    fs::write(&sample_path, b"fake").unwrap();

    let pool = make_pool(&dir).await;
    sonoscope_lib::library::open::open_or_create_library(dir.path().to_str().unwrap(), &pool)
        .await
        .unwrap();
    sonoscope_lib::library::discover::run_discovery(dir.path(), &pool, |_| {})
        .await
        .unwrap();

    let (sample_id,): (i64,) = sqlx::query_as("SELECT id FROM samples LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE samples SET duration_ms = 250, waveform_data = ? WHERE id = ?")
        .bind(vec![0_u8, 128, 255])
        .bind(sample_id)
        .execute(&pool)
        .await
        .unwrap();
    let (loop_value_id,): (i64,) =
        sqlx::query_as("SELECT id FROM dimension_values WHERE dimension_id = 1 AND value = 'loop'")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query(
        "INSERT INTO tags
         (sample_id, dimension_id, value_id, source, confidence, created_at, is_primary)
         VALUES (?, 1, ?, 'heuristic', 0.9, 1, 1)",
    )
    .bind(sample_id)
    .bind(loop_value_id)
    .execute(&pool)
    .await
    .unwrap();

    let playback = sonoscope_lib::commands::playback_sample(&pool, dir.path(), sample_id)
        .await
        .unwrap();

    assert_eq!(playback.id, sample_id);
    assert_eq!(playback.filename, "kick.wav");
    assert_eq!(playback.duration_ms, Some(250));
    assert_eq!(playback.waveform_data, Some(vec![0, 128, 255]));
    assert!(playback.is_loop);
    assert_eq!(
        playback.path,
        sample_path.canonicalize().unwrap().to_string_lossy()
    );
}

#[tokio::test]
async fn test_playback_sample_rejects_paths_outside_library_root() {
    let dir = TempDir::new().unwrap();
    let outside_dir = TempDir::new().unwrap();
    let outside_path = outside_dir.path().join("outside.wav");
    fs::write(&outside_path, b"fake").unwrap();

    let pool = make_pool(&dir).await;
    sonoscope_lib::library::open::open_or_create_library(dir.path().to_str().unwrap(), &pool)
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO samples
         (path, filename, relative_path, format, size_bytes, analysis_status, discovered_at, last_seen_at)
         VALUES (?, 'outside.wav', 'outside.wav', 'wav', 4, 'pending', 1, 1)",
    )
    .bind(outside_path.to_string_lossy().to_string())
    .execute(&pool)
    .await
    .unwrap();

    let (sample_id,): (i64,) = sqlx::query_as("SELECT id FROM samples LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();

    let result = sonoscope_lib::commands::playback_sample(&pool, dir.path(), sample_id).await;

    assert!(matches!(
        result,
        Err(sonoscope_lib::error::CommandError::Other(message))
            if message == "Sample is outside the opened library"
    ));
}

#[tokio::test]
async fn test_sample_rows_returns_tags_and_conflicts_in_bulk() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("loop.wav"), b"fake").unwrap();
    fs::write(dir.path().join("kick.wav"), b"fake").unwrap();

    let pool = make_pool(&dir).await;
    sonoscope_lib::library::open::open_or_create_library(dir.path().to_str().unwrap(), &pool)
        .await
        .unwrap();
    sonoscope_lib::library::discover::run_discovery(dir.path(), &pool, |_| {})
        .await
        .unwrap();

    let (conflicted_id,): (i64,) =
        sqlx::query_as("SELECT id FROM samples WHERE filename = 'loop.wav'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let (loop_value_id,): (i64,) =
        sqlx::query_as("SELECT id FROM dimension_values WHERE dimension_id = 1 AND value = 'loop'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let (one_shot_value_id,): (i64,) = sqlx::query_as(
        "SELECT id FROM dimension_values WHERE dimension_id = 1 AND value = 'one-shot'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO tags
         (sample_id, dimension_id, value_id, source, confidence, created_at, is_primary)
         VALUES (?, 1, ?, 'heuristic', 0.8, 1, 1), (?, 1, ?, 'model', 0.7, 1, 0)",
    )
    .bind(conflicted_id)
    .bind(loop_value_id)
    .bind(conflicted_id)
    .bind(one_shot_value_id)
    .execute(&pool)
    .await
    .unwrap();

    let rows = sonoscope_lib::commands::sample_rows(&pool).await.unwrap();
    assert_eq!(rows.len(), 2);

    let conflicted = rows
        .iter()
        .find(|row| row.id == conflicted_id)
        .expect("conflicted sample present");
    assert_eq!(conflicted.tags.len(), 2);
    assert!(conflicted.tags[0].is_primary, "primary tag ordered first");
    assert_eq!(conflicted.conflicts.len(), 1);
    assert_eq!(conflicted.conflicts[0].dimension, "Type");
    assert_eq!(conflicted.conflicts[0].candidates.len(), 2);

    let clean = rows
        .iter()
        .find(|row| row.id != conflicted_id)
        .expect("clean sample present");
    assert!(clean.tags.is_empty());
    assert!(clean.conflicts.is_empty());

    // Bulk and single-sample paths must agree.
    let single = sonoscope_lib::commands::conflicts_for_sample(&pool, conflicted_id)
        .await
        .unwrap();
    assert_eq!(single.len(), 1);
    assert_eq!(single[0].candidates.len(), 2);
}

#[tokio::test]
async fn test_requeue_untagged_samples_targets_missing_type_or_instrument() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("tagged.wav"), b"fake").unwrap();
    fs::write(dir.path().join("type_only.wav"), b"fake").unwrap();
    fs::write(dir.path().join("mystery.wav"), b"fake").unwrap();

    let pool = make_pool(&dir).await;
    sonoscope_lib::library::open::open_or_create_library(dir.path().to_str().unwrap(), &pool)
        .await
        .unwrap();
    sonoscope_lib::library::discover::run_discovery(dir.path(), &pool, |_| {})
        .await
        .unwrap();
    sqlx::query("UPDATE samples SET analysis_status = 'done'")
        .execute(&pool)
        .await
        .unwrap();

    let (tagged_id,): (i64,) = sqlx::query_as("SELECT id FROM samples WHERE filename = 'tagged.wav'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let (type_only_id,): (i64,) =
        sqlx::query_as("SELECT id FROM samples WHERE filename = 'type_only.wav'")
            .fetch_one(&pool)
            .await
            .unwrap();
    sonoscope_lib::commands::write_user_tag(&pool, tagged_id, "Type", "loop")
        .await
        .unwrap();
    sonoscope_lib::commands::write_user_tag(&pool, tagged_id, "Instrument", "drums")
        .await
        .unwrap();
    sonoscope_lib::commands::write_user_tag(&pool, type_only_id, "Type", "loop")
        .await
        .unwrap();

    sonoscope_lib::analysis::requeue_untagged_samples(&pool)
        .await
        .unwrap();

    let pending: Vec<(String,)> = sqlx::query_as(
        "SELECT filename FROM samples WHERE analysis_status = 'pending' ORDER BY filename",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        pending,
        vec![("mystery.wav".to_string(),), ("type_only.wav".to_string(),)],
        "samples missing Type or Instrument are requeued; fully tagged samples stay done"
    );
}
