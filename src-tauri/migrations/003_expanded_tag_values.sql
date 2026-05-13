INSERT OR IGNORE INTO dimension_values (dimension_id, value, is_system)
VALUES
    ((SELECT id FROM dimensions WHERE name = 'Type'), 'fill', 1),
    ((SELECT id FROM dimensions WHERE name = 'Type'), 'break', 1),
    ((SELECT id FROM dimensions WHERE name = 'Type'), 'top-loop', 1),
    ((SELECT id FROM dimensions WHERE name = 'Type'), 'texture', 1),
    ((SELECT id FROM dimensions WHERE name = 'Instrument'), 'cymbal', 1),
    ((SELECT id FROM dimensions WHERE name = 'Instrument'), 'guitar', 1),
    ((SELECT id FROM dimensions WHERE name = 'Instrument'), 'piano', 1),
    ((SELECT id FROM dimensions WHERE name = 'Instrument'), 'brass', 1),
    ((SELECT id FROM dimensions WHERE name = 'Instrument'), 'woodwind', 1),
    ((SELECT id FROM dimensions WHERE name = 'Instrument'), 'strings', 1);
