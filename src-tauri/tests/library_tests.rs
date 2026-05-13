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
