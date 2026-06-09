-- terrain.coastline: the land/sea shoreline from the N50 "Arealdekke" theme
-- (`kystkontur`). A line layer — the crisp coast edge drawn over the sea
-- (`havflate`, already in terrain.water_polygon as kind='sea'). Geometry
-- EPSG:25833 to match the rest of terrain.*.

CREATE TABLE terrain.coastline (
    id              bigserial PRIMARY KEY,
    geom            geometry(LineString, 25833) NOT NULL,
    source          text NOT NULL,                  -- 'n50'
    attrs           jsonb NOT NULL DEFAULT '{}'::jsonb,
    ingested_at     timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX coastline_geom_gix ON terrain.coastline USING GIST (geom);
