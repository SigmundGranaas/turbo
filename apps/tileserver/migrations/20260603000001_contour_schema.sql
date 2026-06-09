-- terrain.contour: elevation contour lines from the N50 Kartdata "Høyde"
-- theme. Three source object types land in one canonical table:
--   Høydekurve         → kind='main'        (20 m equidistance)
--   Hjelpekurve        → kind='auxiliary'   (half-interval helper curves)
--   Forsenkningskurve  → kind='depression'  (closed depressions)
--
-- `is_index` marks the bold index contour every 100 m (5th line at the
-- 20 m base interval) so the basemap style can render it heavier.
--
-- Geometry EPSG:25833 to match the rest of terrain.* / paths.*.

CREATE TABLE terrain.contour (
    id              bigserial PRIMARY KEY,
    geom            geometry(LineString, 25833) NOT NULL,
    elev_m          integer NOT NULL,
    kind            text NOT NULL,                  -- main|auxiliary|depression
    is_index        boolean NOT NULL DEFAULT false,
    source          text NOT NULL,                  -- 'n50'
    attrs           jsonb NOT NULL DEFAULT '{}'::jsonb,
    ingested_at     timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX contour_geom_gix ON terrain.contour USING GIST (geom);
CREATE INDEX contour_elev_idx ON terrain.contour (elev_m);
CREATE INDEX contour_kind_idx ON terrain.contour (kind);
