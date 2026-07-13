//! Reorganisation of library files by tag pattern: preview planning, move and
//! copy execution with per-file operation records, batch history, rollback,
//! and organisation pattern presets.

pub mod pattern;

use crate::error::CommandError;
use crate::tags::unix_now;
use pattern::{OrganisePattern, UNTAGGED_FOLDER};
use serde::{Deserialize, Serialize};
use specta::Type;
use specta_typescript::Number;
use sqlx::SqlitePool;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Type, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
pub enum OrganiseMode {
    Move,
    Copy,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Type, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
pub enum BatchStatus {
    Completed,
    RolledBack,
}

#[derive(Debug, Serialize, Type)]
pub struct OrganisePlanEntry {
    #[specta(type = Number<i64>)]
    pub sample_id: i64,
    pub from: String,
    pub to: String,
    /// The sample is missing a tag required by the pattern and falls back to `_untagged`.
    pub untagged: bool,
    /// Another sample in the same plan resolves to the same target path.
    pub conflict: bool,
    /// The sample is already at its target path (relevant in move mode).
    pub unchanged: bool,
}

#[derive(Debug, Serialize, Type)]
pub struct OrganisePreview {
    pub entries: Vec<OrganisePlanEntry>,
    pub total: u32,
    pub untagged_count: u32,
    pub conflict_count: u32,
    pub unchanged_count: u32,
}

#[derive(Debug, Serialize, Type)]
pub struct OrganiseApplyResult {
    #[specta(type = Option<Number<i64>>)]
    pub batch_id: Option<i64>,
    pub processed: u32,
    pub skipped: u32,
    pub total: u32,
    /// First few per-file error messages, for surfacing in the UI.
    pub errors: Vec<String>,
}

#[derive(Debug, Serialize, Type)]
pub struct RollbackResult {
    pub restored: u32,
    pub skipped: u32,
}

#[derive(Debug, Serialize, Type)]
pub struct OperationBatch {
    #[specta(type = Number<i64>)]
    pub id: i64,
    #[specta(type = Number<i64>)]
    pub created_at: i64,
    pub pattern: String,
    pub mode: OrganiseMode,
    #[specta(type = Number<i64>)]
    pub file_count: i64,
    pub status: BatchStatus,
}

#[derive(Debug, Serialize, Type)]
pub struct OrganisationPreset {
    #[specta(type = Number<i64>)]
    pub id: i64,
    pub name: String,
    pub pattern: String,
    pub is_system: bool,
}

/// One planned file operation, with the source's absolute path kept for apply.
#[derive(Debug)]
struct PlannedOp {
    sample_id: i64,
    source_abs: PathBuf,
    from_rel: String,
    to_rel: String,
    untagged: bool,
    conflict: bool,
    unchanged: bool,
}

const MAX_REPORTED_ERRORS: usize = 10;
const PROGRESS_INTERVAL: u32 = 25;

pub async fn preview_organise(
    pool: &SqlitePool,
    pattern_str: &str,
    sample_ids: Option<&[i64]>,
) -> Result<OrganisePreview, CommandError> {
    let plan = build_plan(pool, pattern_str, sample_ids).await?;

    let mut preview = OrganisePreview {
        total: plan.len() as u32,
        entries: Vec::with_capacity(plan.len()),
        untagged_count: 0,
        conflict_count: 0,
        unchanged_count: 0,
    };
    for op in plan {
        preview.untagged_count += op.untagged as u32;
        preview.conflict_count += op.conflict as u32;
        preview.unchanged_count += op.unchanged as u32;
        preview.entries.push(OrganisePlanEntry {
            sample_id: op.sample_id,
            from: op.from_rel,
            to: op.to_rel,
            untagged: op.untagged,
            conflict: op.conflict,
            unchanged: op.unchanged,
        });
    }
    Ok(preview)
}

pub async fn apply_organise(
    pool: &SqlitePool,
    library_root: &Path,
    pattern_str: &str,
    mode: OrganiseMode,
    destination: Option<&Path>,
    sample_ids: Option<&[i64]>,
    on_progress: impl Fn(u32, u32),
) -> Result<OrganiseApplyResult, CommandError> {
    let destination_root = match mode {
        OrganiseMode::Move => library_root.to_path_buf(),
        OrganiseMode::Copy => {
            let destination = destination.ok_or_else(|| {
                CommandError::Other("Copy mode requires a destination folder".to_string())
            })?;
            validate_copy_destination(library_root, destination)?
        }
    };

    let plan = build_plan(pool, pattern_str, sample_ids).await?;
    let total = plan.len() as u32;
    let mut result = OrganiseApplyResult {
        batch_id: None,
        processed: 0,
        skipped: 0,
        total,
        errors: Vec::new(),
    };

    for op in &plan {
        if op.conflict || (matches!(mode, OrganiseMode::Move) && op.unchanged) {
            result.skipped += 1;
            continue;
        }

        match execute_op(pool, &mut result, library_root, &destination_root, op, mode, pattern_str)
            .await
        {
            Ok(true) => result.processed += 1,
            Ok(false) => result.skipped += 1,
            Err(e) => {
                result.skipped += 1;
                if result.errors.len() < MAX_REPORTED_ERRORS {
                    result.errors.push(format!("{}: {e}", op.from_rel));
                }
            }
        }

        let done = result.processed + result.skipped;
        if done % PROGRESS_INTERVAL == 0 {
            on_progress(done, total);
        }
    }
    on_progress(result.processed + result.skipped, total);

    if let Some(batch_id) = result.batch_id {
        let file_count = result.processed as i64;
        sqlx::query!(
            "UPDATE operation_batches SET file_count = ? WHERE id = ?",
            file_count,
            batch_id,
        )
        .execute(pool)
        .await?;
    }

    Ok(result)
}

/// Perform the filesystem operation and record it. Returns `Ok(false)` when
/// the file was skipped because the target already exists.
#[allow(clippy::too_many_arguments)]
async fn execute_op(
    pool: &SqlitePool,
    result: &mut OrganiseApplyResult,
    library_root: &Path,
    destination_root: &Path,
    op: &PlannedOp,
    mode: OrganiseMode,
    pattern_str: &str,
) -> Result<bool, CommandError> {
    let target_abs = destination_root.join(&op.to_rel);
    if tokio::fs::try_exists(&target_abs).await? {
        return Ok(false);
    }
    if let Some(parent) = target_abs.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    match mode {
        OrganiseMode::Move => tokio::fs::rename(&op.source_abs, &target_abs).await?,
        OrganiseMode::Copy => {
            tokio::fs::copy(&op.source_abs, &target_abs).await?;
        }
    }

    let batch_id = match result.batch_id {
        Some(id) => id,
        None => {
            let id = create_batch(pool, pattern_str, mode).await?;
            result.batch_id = Some(id);
            id
        }
    };

    let now = unix_now();
    let original_path = op.source_abs.to_string_lossy().to_string();
    let new_path = target_abs.to_string_lossy().to_string();
    let mut tx = pool.begin().await?;
    sqlx::query!(
        "INSERT INTO file_operations
         (batch_id, sample_id, operation_type, original_path, new_path, executed_at)
         VALUES (?, ?, ?, ?, ?, ?)",
        batch_id,
        op.sample_id,
        mode,
        original_path,
        new_path,
        now,
    )
    .execute(&mut *tx)
    .await?;
    if matches!(mode, OrganiseMode::Move) {
        let new_relative = relative_to_root(library_root, &target_abs);
        sqlx::query!(
            "UPDATE samples SET path = ?, relative_path = ? WHERE id = ?",
            new_path,
            new_relative,
            op.sample_id,
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    Ok(true)
}

pub async fn rollback_batch(
    pool: &SqlitePool,
    batch_id: i64,
) -> Result<RollbackResult, CommandError> {
    let batch = sqlx::query!(
        "SELECT mode as \"mode: OrganiseMode\", status as \"status: BatchStatus\"
         FROM operation_batches WHERE id = ?",
        batch_id,
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| CommandError::Other("Unknown operation batch".to_string()))?;

    if !matches!(batch.mode, OrganiseMode::Move) {
        return Err(CommandError::Other(
            "Only move batches can be rolled back".to_string(),
        ));
    }
    if !matches!(batch.status, BatchStatus::Completed) {
        return Err(CommandError::Other(
            "Batch is already rolled back".to_string(),
        ));
    }

    let root = sqlx::query!("SELECT root_path FROM library_meta WHERE id = 1")
        .fetch_one(pool)
        .await?
        .root_path;
    let root = PathBuf::from(root);

    let operations = sqlx::query!(
        "SELECT sample_id as \"sample_id!: i64\", original_path, new_path
         FROM file_operations WHERE batch_id = ? ORDER BY id DESC",
        batch_id,
    )
    .fetch_all(pool)
    .await?;

    let mut result = RollbackResult {
        restored: 0,
        skipped: 0,
    };
    for operation in operations {
        let current = PathBuf::from(&operation.new_path);
        let original = PathBuf::from(&operation.original_path);
        let movable = tokio::fs::try_exists(&current).await.unwrap_or(false)
            && !tokio::fs::try_exists(&original).await.unwrap_or(true);
        if !movable {
            result.skipped += 1;
            continue;
        }

        if let Some(parent) = original.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::rename(&current, &original).await?;

        let original_relative = relative_to_root(&root, &original);
        sqlx::query!(
            "UPDATE samples SET path = ?, relative_path = ? WHERE id = ?",
            operation.original_path,
            original_relative,
            operation.sample_id,
        )
        .execute(pool)
        .await?;
        result.restored += 1;
    }

    let rolled_back = BatchStatus::RolledBack;
    sqlx::query!(
        "UPDATE operation_batches SET status = ? WHERE id = ?",
        rolled_back,
        batch_id,
    )
    .execute(pool)
    .await?;

    Ok(result)
}

pub async fn list_batches(pool: &SqlitePool) -> Result<Vec<OperationBatch>, CommandError> {
    let rows = sqlx::query!(
        "SELECT id as \"id!: i64\", created_at, pattern,
                mode as \"mode: OrganiseMode\", file_count,
                status as \"status: BatchStatus\"
         FROM operation_batches ORDER BY created_at DESC, id DESC",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| OperationBatch {
            id: row.id,
            created_at: row.created_at,
            pattern: row.pattern,
            mode: row.mode,
            file_count: row.file_count,
            status: row.status,
        })
        .collect())
}

pub async fn list_presets(pool: &SqlitePool) -> Result<Vec<OrganisationPreset>, CommandError> {
    let rows = sqlx::query!(
        "SELECT id as \"id!: i64\", name, pattern, is_system as \"is_system: bool\"
         FROM organisation_presets ORDER BY is_system DESC, name",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| OrganisationPreset {
            id: row.id,
            name: row.name,
            pattern: row.pattern,
            is_system: row.is_system,
        })
        .collect())
}

pub async fn save_preset(
    pool: &SqlitePool,
    name: &str,
    pattern_str: &str,
) -> Result<OrganisationPreset, CommandError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(CommandError::Other("Preset name is empty".to_string()));
    }
    OrganisePattern::parse(pattern_str)?;

    let now = unix_now();
    sqlx::query!(
        "INSERT INTO organisation_presets (name, pattern, is_system, created_at)
         VALUES (?, ?, 0, ?)
         ON CONFLICT(name) DO UPDATE SET pattern = excluded.pattern",
        name,
        pattern_str,
        now,
    )
    .execute(pool)
    .await?;

    let row = sqlx::query!(
        "SELECT id as \"id!: i64\", name, pattern, is_system as \"is_system: bool\"
         FROM organisation_presets WHERE name = ?",
        name,
    )
    .fetch_one(pool)
    .await?;

    Ok(OrganisationPreset {
        id: row.id,
        name: row.name,
        pattern: row.pattern,
        is_system: row.is_system,
    })
}

pub async fn delete_preset(pool: &SqlitePool, preset_id: i64) -> Result<(), CommandError> {
    sqlx::query!("DELETE FROM organisation_presets WHERE id = ?", preset_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Resolve every sample (or the given subset) against the pattern into a
/// planned target path, flagging untagged fallbacks, in-plan target
/// collisions, and files already at their target.
async fn build_plan(
    pool: &SqlitePool,
    pattern_str: &str,
    sample_ids: Option<&[i64]>,
) -> Result<Vec<PlannedOp>, CommandError> {
    let pattern = OrganisePattern::parse(pattern_str)?;

    let known_dimensions: HashSet<String> = sqlx::query!("SELECT name FROM dimensions")
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|row| row.name)
        .collect();
    for dimension in pattern.dimensions() {
        if !known_dimensions.contains(dimension) {
            return Err(CommandError::Other(format!(
                "Unknown dimension in pattern: {dimension}"
            )));
        }
    }

    let samples = sqlx::query!(
        "SELECT id as \"id!: i64\", path, filename, relative_path
         FROM samples ORDER BY relative_path",
    )
    .fetch_all(pool)
    .await?;
    let id_filter: Option<HashSet<i64>> = sample_ids.map(|ids| ids.iter().copied().collect());

    let primary_tags = load_primary_tags(pool).await?;

    let mut plan = Vec::new();
    let mut seen_targets: HashSet<String> = HashSet::new();
    for sample in samples {
        if let Some(filter) = &id_filter {
            if !filter.contains(&sample.id) {
                continue;
            }
        }

        let empty = HashMap::new();
        let tags = primary_tags.get(&sample.id).unwrap_or(&empty);
        let (folders, untagged) = match pattern.resolve(tags) {
            Some(folders) => (folders, false),
            None => (vec![UNTAGGED_FOLDER.to_string()], true),
        };

        let mut to_rel = PathBuf::new();
        for folder in &folders {
            to_rel.push(folder);
        }
        to_rel.push(&sample.filename);
        let to_rel_str = to_rel.to_string_lossy().to_string();

        let normalized_from = normalize_for_compare(&sample.relative_path);
        let normalized_to = normalize_for_compare(&to_rel_str);
        let unchanged = normalized_from == normalized_to;
        let conflict = !seen_targets.insert(normalized_to);

        plan.push(PlannedOp {
            sample_id: sample.id,
            source_abs: PathBuf::from(&sample.path),
            from_rel: sample.relative_path,
            to_rel: to_rel_str,
            untagged,
            conflict,
            unchanged,
        });
    }

    Ok(plan)
}

/// Primary tag display value per sample and dimension, formatted the same way
/// the review UI shows it (whole-number tempos without a trailing `.0`).
async fn load_primary_tags(
    pool: &SqlitePool,
) -> Result<HashMap<i64, HashMap<String, String>>, CommandError> {
    let rows = sqlx::query!(
        "SELECT t.sample_id as \"sample_id!: i64\",
                d.name as dimension,
                dv.value as \"enum_value?: String\",
                t.numeric_value,
                t.text_value
         FROM tags t
         JOIN dimensions d ON d.id = t.dimension_id
         LEFT JOIN dimension_values dv ON dv.id = t.value_id
         WHERE t.is_primary = 1",
    )
    .fetch_all(pool)
    .await?;

    let mut tags: HashMap<i64, HashMap<String, String>> = HashMap::new();
    for row in rows {
        let value = match (row.enum_value, row.numeric_value, row.text_value) {
            (Some(value), _, _) => value,
            (None, Some(numeric), _) => format_numeric(numeric),
            (None, None, Some(text)) => text,
            (None, None, None) => continue,
        };
        tags.entry(row.sample_id).or_default().insert(row.dimension, value);
    }
    Ok(tags)
}

fn format_numeric(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

fn normalize_for_compare(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

fn relative_to_root(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

fn validate_copy_destination(
    library_root: &Path,
    destination: &Path,
) -> Result<PathBuf, CommandError> {
    let destination = destination.canonicalize().map_err(|_| {
        CommandError::Other("Copy destination folder does not exist".to_string())
    })?;
    if !destination.is_dir() {
        return Err(CommandError::Other(
            "Copy destination is not a folder".to_string(),
        ));
    }
    if let Ok(canonical_root) = library_root.canonicalize() {
        if destination.starts_with(&canonical_root) {
            return Err(CommandError::Other(
                "Copy destination must be outside the library".to_string(),
            ));
        }
    }
    Ok(destination)
}

async fn create_batch(
    pool: &SqlitePool,
    pattern_str: &str,
    mode: OrganiseMode,
) -> Result<i64, CommandError> {
    let now = unix_now();
    let status = BatchStatus::Completed;
    let result = sqlx::query!(
        "INSERT INTO operation_batches (created_at, pattern, mode, file_count, status)
         VALUES (?, ?, ?, 0, ?)",
        now,
        pattern_str,
        mode,
        status,
    )
    .execute(pool)
    .await?;
    Ok(result.last_insert_rowid())
}

#[cfg(test)]
mod tests {
    use super::{format_numeric, normalize_for_compare};

    #[test]
    fn numeric_tags_format_without_trailing_zero() {
        assert_eq!(format_numeric(120.0), "120");
        assert_eq!(format_numeric(117.5), "117.5");
    }

    #[test]
    fn path_comparison_ignores_separator_and_case() {
        assert_eq!(
            normalize_for_compare("Loops\\Bass\\a.wav"),
            normalize_for_compare("loops/bass/A.WAV"),
        );
    }
}
