INSERT OR IGNORE INTO dimensions (name, value_type, is_system, sort_order)
VALUES ('Mode', 'enum', 1, 35);

INSERT OR IGNORE INTO dimension_values (dimension_id, value, is_system)
VALUES
    ((SELECT id FROM dimensions WHERE name = 'Mode'), 'major', 1),
    ((SELECT id FROM dimensions WHERE name = 'Mode'), 'minor', 1);
