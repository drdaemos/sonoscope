CREATE TABLE dimensions (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    value_type TEXT NOT NULL,
    is_system INTEGER NOT NULL,
    sort_order INTEGER NOT NULL
);

CREATE TABLE dimension_values (
    id INTEGER PRIMARY KEY,
    dimension_id INTEGER NOT NULL,
    value TEXT NOT NULL,
    is_system INTEGER NOT NULL,
    FOREIGN KEY (dimension_id) REFERENCES dimensions(id) ON DELETE CASCADE,
    UNIQUE (dimension_id, value)
);

CREATE TABLE tags (
    id INTEGER PRIMARY KEY,
    sample_id INTEGER NOT NULL,
    dimension_id INTEGER NOT NULL,
    value_id INTEGER,
    numeric_value REAL,
    text_value TEXT,
    source TEXT NOT NULL,
    confidence REAL,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (sample_id) REFERENCES samples(id) ON DELETE CASCADE,
    FOREIGN KEY (dimension_id) REFERENCES dimensions(id) ON DELETE CASCADE,
    FOREIGN KEY (value_id) REFERENCES dimension_values(id) ON DELETE CASCADE,
    UNIQUE (sample_id, dimension_id, value_id, source)
);

CREATE INDEX idx_tags_sample_dimension ON tags(sample_id, dimension_id);
CREATE INDEX idx_tags_dimension_value ON tags(dimension_id, value_id);

INSERT INTO dimensions (id, name, value_type, is_system, sort_order)
VALUES
    (1, 'Type', 'enum', 1, 10),
    (2, 'Instrument', 'multi_enum', 1, 20),
    (3, 'Key', 'enum', 1, 30),
    (4, 'Tempo', 'numeric', 1, 40),
    (5, 'Mood', 'multi_enum', 1, 50);

INSERT INTO dimension_values (dimension_id, value, is_system)
VALUES
    (1, 'loop', 1),
    (1, 'one-shot', 1),
    (2, 'kick', 1),
    (2, 'snare', 1),
    (2, 'hi-hat', 1),
    (2, 'clap', 1),
    (2, 'percussion', 1),
    (2, 'bass', 1),
    (2, 'chord', 1),
    (2, 'pad', 1),
    (2, 'synth', 1),
    (2, 'lead', 1),
    (2, 'vocal', 1),
    (2, 'fx', 1),
    (2, 'foley', 1),
    (3, 'A', 1),
    (3, 'A#', 1),
    (3, 'B', 1),
    (3, 'C', 1),
    (3, 'C#', 1),
    (3, 'D', 1),
    (3, 'D#', 1),
    (3, 'E', 1),
    (3, 'F', 1),
    (3, 'F#', 1),
    (3, 'G', 1),
    (3, 'G#', 1);
