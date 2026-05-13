ALTER TABLE tags ADD COLUMN is_primary INTEGER NOT NULL DEFAULT 0;

CREATE INDEX idx_tags_sample_dimension_primary ON tags(sample_id, dimension_id, is_primary);
