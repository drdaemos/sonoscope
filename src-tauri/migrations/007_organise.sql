CREATE TABLE operation_batches (
    id INTEGER PRIMARY KEY,
    created_at INTEGER NOT NULL,
    pattern TEXT NOT NULL,
    mode TEXT NOT NULL,
    file_count INTEGER NOT NULL,
    status TEXT NOT NULL
);

CREATE TABLE file_operations (
    id INTEGER PRIMARY KEY,
    batch_id INTEGER NOT NULL,
    sample_id INTEGER NOT NULL,
    operation_type TEXT NOT NULL,
    original_path TEXT NOT NULL,
    new_path TEXT NOT NULL,
    executed_at INTEGER NOT NULL,
    FOREIGN KEY (batch_id) REFERENCES operation_batches(id) ON DELETE CASCADE,
    FOREIGN KEY (sample_id) REFERENCES samples(id) ON DELETE CASCADE
);

CREATE INDEX idx_file_operations_batch_id ON file_operations(batch_id);

CREATE TABLE organisation_presets (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    pattern TEXT NOT NULL,
    is_system INTEGER NOT NULL,
    created_at INTEGER NOT NULL
);

INSERT INTO organisation_presets (name, pattern, is_system, created_at)
VALUES
    ('Type / Instrument', '{Type}/{Instrument}', 1, strftime('%s', 'now')),
    ('Instrument / Type', '{Instrument}/{Type}', 1, strftime('%s', 'now'));
