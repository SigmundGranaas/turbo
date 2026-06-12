-- terrain.building_polygon: building footprints from the N50
-- "BygningerOgAnlegg" theme (`bygning_omrade`). The defining z14+ basemap
-- layer. Geometry EPSG:25833 to match the rest of terrain.*.
--
-- NB: `tools/vector-layers.toml` already exposes `n50_staging.bygning_omrade`
-- directly as a routing refusal layer; this canonical table is the basemap
-- serve path (and survives the staging schema being dropped).

CREATE TABLE terrain.building_polygon (
    id              bigserial PRIMARY KEY,
    geom            geometry(MultiPolygon, 25833) NOT NULL,
    building_type   text,                           -- N50 `bygningstype`
    name            text,                           -- N50 `navn`, when present
    source          text NOT NULL,                  -- 'n50'
    attrs           jsonb NOT NULL DEFAULT '{}'::jsonb,
    ingested_at     timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX building_polygon_geom_gix ON terrain.building_polygon USING GIST (geom);
CREATE INDEX building_polygon_type_idx ON terrain.building_polygon (building_type);
