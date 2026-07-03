//! Shared tag write logic: candidate inserts, user tags, and primary-tag
//! promotion. Both the Tauri commands and the analysis pipeline go through
//! this module so the primary-tag rules cannot diverge.

use crate::error::{CommandError, DimensionValueType, TagSource};
use sqlx::{SqliteConnection, SqlitePool};
use std::collections::HashMap;

pub fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

#[derive(Debug, Clone)]
pub struct DimensionRef {
    pub id: i64,
    pub value_type: DimensionValueType,
}

/// Lookup tables loaded once per analysis run so tag inserts do not re-query
/// the `dimensions` and `dimension_values` tables for every tag.
#[derive(Debug, Clone, Default)]
pub struct TagLookup {
    pub dimensions: HashMap<String, DimensionRef>,
    value_ids: HashMap<(i64, String), i64>,
}

impl TagLookup {
    pub fn dimension(&self, name: &str) -> Option<&DimensionRef> {
        self.dimensions.get(name)
    }

    pub fn value_id(&self, dimension_id: i64, value: &str) -> Option<i64> {
        self.value_ids
            .get(&(dimension_id, value.to_string()))
            .copied()
    }
}

pub async fn load_tag_lookup(pool: &SqlitePool) -> Result<TagLookup, CommandError> {
    let dimensions = sqlx::query!(
        "SELECT id as \"id!: i64\", name, value_type as \"value_type: DimensionValueType\" FROM dimensions",
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| {
        (
            row.name,
            DimensionRef {
                id: row.id,
                value_type: row.value_type,
            },
        )
    })
    .collect();

    let value_ids = sqlx::query!(
        "SELECT id as \"id!: i64\", dimension_id as \"dimension_id!: i64\", value FROM dimension_values",
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| ((row.dimension_id, row.value), row.id))
    .collect();

    Ok(TagLookup {
        dimensions,
        value_ids,
    })
}

/// Insert one non-user tag candidate. Unknown dimensions and unknown enum
/// values are skipped silently — the analyzer may emit tags the library
/// schema does not define.
pub async fn insert_auto_tag(
    conn: &mut SqliteConnection,
    lookup: &TagLookup,
    sample_id: i64,
    dimension_name: &str,
    value: &str,
    source: &TagSource,
    confidence: f64,
) -> Result<(), CommandError> {
    let Some(dimension) = lookup.dimension(dimension_name) else {
        return Ok(());
    };
    let dimension_id = dimension.id;

    let now = unix_now();
    match dimension.value_type {
        DimensionValueType::Enum | DimensionValueType::MultiEnum => {
            if let Some(value_id) = lookup.value_id(dimension_id, value) {
                sqlx::query!(
                    "INSERT OR IGNORE INTO tags
                     (sample_id, dimension_id, value_id, source, confidence, created_at, is_primary)
                     VALUES (?, ?, ?, ?, ?, ?, 0)",
                    sample_id,
                    dimension_id,
                    value_id,
                    source,
                    confidence,
                    now,
                )
                .execute(&mut *conn)
                .await?;
            }
        }
        DimensionValueType::Numeric => {
            if let Ok(numeric_value) = value.parse::<f64>() {
                sqlx::query!(
                    "INSERT INTO tags
                     (sample_id, dimension_id, numeric_value, source, confidence, created_at, is_primary)
                     VALUES (?, ?, ?, ?, ?, ?, 0)",
                    sample_id,
                    dimension_id,
                    numeric_value,
                    source,
                    confidence,
                    now,
                )
                .execute(&mut *conn)
                .await?;
            }
        }
        DimensionValueType::Text => {
            sqlx::query!(
                "INSERT INTO tags
                 (sample_id, dimension_id, text_value, source, confidence, created_at, is_primary)
                 VALUES (?, ?, ?, ?, ?, ?, 0)",
                sample_id,
                dimension_id,
                value,
                source,
                confidence,
                now,
            )
            .execute(&mut *conn)
            .await?;
        }
    }

    Ok(())
}

pub async fn write_user_tag(
    pool: &SqlitePool,
    sample_id: i64,
    dimension_name: &str,
    value: &str,
) -> Result<(), CommandError> {
    let dimension = sqlx::query!(
        "SELECT id as \"id!: i64\", value_type as \"value_type: DimensionValueType\" FROM dimensions WHERE name = ?",
        dimension_name,
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| CommandError::Other(format!("Unknown dimension: {dimension_name}")))?;

    let mut tx = pool.begin().await?;

    if !matches!(dimension.value_type, DimensionValueType::MultiEnum) {
        delete_user_tags_for_dimension(&mut *tx, sample_id, dimension.id).await?;
    }
    clear_all_primary_tags_for_dimension(&mut *tx, sample_id, dimension.id).await?;

    let user_source = TagSource::User;
    let now = unix_now();
    match dimension.value_type {
        DimensionValueType::Enum | DimensionValueType::MultiEnum => {
            let value_row = sqlx::query!(
                "SELECT id as \"id!: i64\" FROM dimension_values WHERE dimension_id = ? AND value = ?",
                dimension.id,
                value,
            )
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| {
                CommandError::Other(format!(
                    "Unknown value {value} for dimension {dimension_name}"
                ))
            })?;

            sqlx::query!(
                "INSERT OR IGNORE INTO tags
                 (sample_id, dimension_id, value_id, source, confidence, created_at, is_primary)
                 VALUES (?, ?, ?, ?, NULL, ?, 0)",
                sample_id,
                dimension.id,
                value_row.id,
                user_source,
                now,
            )
            .execute(&mut *tx)
            .await?;
            sqlx::query!(
                "UPDATE tags
                 SET is_primary = 1
                 WHERE sample_id = ?
                   AND dimension_id = ?
                   AND value_id = ?
                   AND source = ?",
                sample_id,
                dimension.id,
                value_row.id,
                user_source,
            )
            .execute(&mut *tx)
            .await?;
        }
        DimensionValueType::Numeric => {
            let numeric_value = value
                .parse::<f64>()
                .map_err(|_| CommandError::Other(format!("{value} is not a valid number")))?;
            sqlx::query!(
                "INSERT INTO tags
                 (sample_id, dimension_id, numeric_value, source, confidence, created_at, is_primary)
                 VALUES (?, ?, ?, ?, NULL, ?, 1)",
                sample_id,
                dimension.id,
                numeric_value,
                user_source,
                now,
            )
            .execute(&mut *tx)
            .await?;
        }
        DimensionValueType::Text => {
            sqlx::query!(
                "INSERT INTO tags
                 (sample_id, dimension_id, text_value, source, confidence, created_at, is_primary)
                 VALUES (?, ?, ?, ?, NULL, ?, 1)",
                sample_id,
                dimension.id,
                value,
                user_source,
                now,
            )
            .execute(&mut *tx)
            .await?;
        }
    }

    tx.commit().await?;
    Ok(())
}

pub async fn clear_user_tag_for_dimension(
    pool: &SqlitePool,
    sample_id: i64,
    dimension_id: i64,
) -> Result<(), CommandError> {
    let mut tx = pool.begin().await?;
    delete_user_tags_for_dimension(&mut *tx, sample_id, dimension_id).await?;
    mark_auto_primary_for_dimension(&mut *tx, sample_id, dimension_id).await?;
    tx.commit().await?;
    Ok(())
}

async fn delete_user_tags_for_dimension(
    conn: &mut SqliteConnection,
    sample_id: i64,
    dimension_id: i64,
) -> Result<(), CommandError> {
    let user_source = TagSource::User;
    sqlx::query!(
        "DELETE FROM tags WHERE sample_id = ? AND dimension_id = ? AND source = ?",
        sample_id,
        dimension_id,
        user_source,
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

async fn clear_all_primary_tags_for_dimension(
    conn: &mut SqliteConnection,
    sample_id: i64,
    dimension_id: i64,
) -> Result<(), CommandError> {
    sqlx::query!(
        "UPDATE tags SET is_primary = 0 WHERE sample_id = ? AND dimension_id = ?",
        sample_id,
        dimension_id,
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Recompute the primary flag for the non-user tags of one dimension. If a
/// user primary exists it wins; otherwise the highest-confidence auto tag is
/// promoted.
pub async fn mark_auto_primary_for_dimension(
    conn: &mut SqliteConnection,
    sample_id: i64,
    dimension_id: i64,
) -> Result<(), CommandError> {
    sqlx::query!(
        "UPDATE tags SET is_primary = 0 WHERE sample_id = ? AND dimension_id = ? AND source != 'user'",
        sample_id,
        dimension_id,
    )
    .execute(&mut *conn)
    .await?;

    let has_user_primary = sqlx::query!(
        "SELECT 1 as \"exists!: i64\"
         FROM tags
         WHERE sample_id = ?
           AND dimension_id = ?
           AND source = 'user'
           AND is_primary = 1
         LIMIT 1",
        sample_id,
        dimension_id,
    )
    .fetch_optional(&mut *conn)
    .await?;
    if has_user_primary.is_some() {
        return Ok(());
    }

    let candidate = sqlx::query!(
        "SELECT id as \"id!: i64\"
         FROM tags
         WHERE sample_id = ?
           AND dimension_id = ?
           AND source != 'user'
         ORDER BY confidence DESC, id ASC
         LIMIT 1",
        sample_id,
        dimension_id,
    )
    .fetch_optional(&mut *conn)
    .await?;
    if let Some(candidate) = candidate {
        sqlx::query!("UPDATE tags SET is_primary = 1 WHERE id = ?", candidate.id)
            .execute(&mut *conn)
            .await?;
    }
    Ok(())
}
