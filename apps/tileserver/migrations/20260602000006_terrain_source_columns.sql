-- Add `source` columns to water + glacier polygon tables so the
-- bulk-ingest jobs can DELETE-and-INSERT scoped to their source
-- label (e.g. "n50" vs "seed"). `terrain.landcover_patch` already
-- has this column; this migration brings the other two in line.

ALTER TABLE terrain.water_polygon
    ADD COLUMN IF NOT EXISTS source text;
ALTER TABLE terrain.glacier_polygon
    ADD COLUMN IF NOT EXISTS source text;

CREATE INDEX IF NOT EXISTS water_polygon_source_idx   ON terrain.water_polygon   (source);
CREATE INDEX IF NOT EXISTS glacier_polygon_source_idx ON terrain.glacier_polygon (source);

-- Backfill any prior rows (from the recommend-seed fixture). The
-- fixture writes `attrs->>'source' = 'seed'`; copy that out.
UPDATE terrain.water_polygon
SET source = attrs->>'source'
WHERE source IS NULL AND attrs ? 'source';
UPDATE terrain.glacier_polygon
SET source = attrs->>'source'
WHERE source IS NULL AND attrs ? 'source';
