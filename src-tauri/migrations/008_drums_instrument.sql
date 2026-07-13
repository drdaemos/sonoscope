INSERT OR IGNORE INTO dimension_values (dimension_id, value, is_system)
VALUES ((SELECT id FROM dimensions WHERE name = 'Instrument'), 'drums', 1);
