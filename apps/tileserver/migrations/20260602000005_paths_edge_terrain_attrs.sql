-- Edge-attached terrain attributes used by the unified routing graph.
-- These are derived once at ingest and never recomputed at query
-- time (Rule R3). When the definition of any of these changes, bump
-- recommend.attr_version and re-run the derivation job.

ALTER TABLE paths.edge
    ADD COLUMN IF NOT EXISTS mean_slope_deg      double precision,
    ADD COLUMN IF NOT EXISTS max_slope_deg       double precision,
    ADD COLUMN IF NOT EXISTS mean_aspect_deg     double precision,
    ADD COLUMN IF NOT EXISTS landcover_class     text,
    ADD COLUMN IF NOT EXISTS landform_class      text,
    ADD COLUMN IF NOT EXISTS exposure_above_treeline_pct double precision,
    ADD COLUMN IF NOT EXISTS attr_version        integer NOT NULL DEFAULT 1;

-- Extend the ingest source enum with 'skeleton' for synthetic
-- off-trail edges from the landform skeleton.
ALTER TYPE paths.ingest_source ADD VALUE IF NOT EXISTS 'skeleton';

CREATE INDEX IF NOT EXISTS edge_landcover_idx  ON paths.edge (landcover_class) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS edge_landform_idx   ON paths.edge (landform_class)  WHERE deleted_at IS NULL;
