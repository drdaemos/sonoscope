PRAGMA journal_mode=WAL;
PRAGMA foreign_keys=ON;

CREATE TABLE IF NOT EXISTS library_meta (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    root_path TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    last_discovered_at INTEGER
);

CREATE TABLE IF NOT EXISTS samples (
    id INTEGER PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    filename TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    format TEXT,
    size_bytes INTEGER,
    duration_ms INTEGER,
    sample_rate INTEGER,
    bit_depth INTEGER,
    channels INTEGER,
    file_hash TEXT,
    analysis_status TEXT NOT NULL DEFAULT 'pending',
    discovered_at INTEGER NOT NULL,
    last_seen_at INTEGER NOT NULL,
    waveform_data BLOB
);

CREATE INDEX IF NOT EXISTS idx_samples_path ON samples(path);
CREATE INDEX IF NOT EXISTS idx_samples_file_hash ON samples(file_hash);
CREATE INDEX IF NOT EXISTS idx_samples_analysis_status ON samples(analysis_status);
CREATE INDEX IF NOT EXISTS idx_samples_filename ON samples(filename);
