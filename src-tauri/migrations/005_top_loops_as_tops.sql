INSERT OR IGNORE INTO dimension_values (dimension_id, value, is_system)
VALUES ((SELECT id FROM dimensions WHERE name = 'Instrument'), 'tops', 1);

DELETE FROM tags
WHERE value_id = (
    SELECT dv.id
    FROM dimension_values dv
    JOIN dimensions d ON d.id = dv.dimension_id
    WHERE d.name = 'Type' AND dv.value = 'top-loop'
)
AND EXISTS (
    SELECT 1
    FROM tags existing
    WHERE existing.sample_id = tags.sample_id
      AND existing.dimension_id = tags.dimension_id
      AND existing.source = tags.source
      AND existing.value_id = (
          SELECT dv.id
          FROM dimension_values dv
          JOIN dimensions d ON d.id = dv.dimension_id
          WHERE d.name = 'Type' AND dv.value = 'loop'
      )
);

UPDATE tags
SET value_id = (
    SELECT dv.id
    FROM dimension_values dv
    JOIN dimensions d ON d.id = dv.dimension_id
    WHERE d.name = 'Type' AND dv.value = 'loop'
)
WHERE value_id = (
    SELECT dv.id
    FROM dimension_values dv
    JOIN dimensions d ON d.id = dv.dimension_id
    WHERE d.name = 'Type' AND dv.value = 'top-loop'
);

DELETE FROM dimension_values
WHERE dimension_id = (SELECT id FROM dimensions WHERE name = 'Type')
  AND value = 'top-loop';
