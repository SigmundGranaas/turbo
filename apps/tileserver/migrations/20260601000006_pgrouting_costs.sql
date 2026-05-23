-- Edge cost columns. Filled lazily by ingest (length-only baseline);
-- per-profile cost is computed at query time in turbo-tiles-routing
-- via the SQL expressions in `profile::cost_expression`. We carry the
-- columns so pgRouting's `pgr_dijkstra` can read them directly without
-- a join when a fixed cost is acceptable.
--
-- Postgres disallows referencing a generated column from another
-- generated column's expression, so the baseline `cost` columns
-- recompute `ST_Length(geom)` directly rather than reusing `length_m`.
--
-- `pgr_createTopology` is invoked by the ingest job, not by this
-- migration — topology rebuild requires a populated table.

ALTER TABLE paths.edge
    ADD COLUMN cost double precision GENERATED ALWAYS AS (ST_Length(geom)) STORED,
    ADD COLUMN reverse_cost double precision GENERATED ALWAYS AS (ST_Length(geom)) STORED;
