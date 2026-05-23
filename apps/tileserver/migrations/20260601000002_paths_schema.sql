-- Routable graph: edges + their endpoint nodes. Geometry stored in
-- EPSG:25833 (ETRS89 / UTM33N — Kartverket native, accurate lengths
-- across mainland Norway). MVT serving transforms to 3857 at query
-- time inside ST_AsMVT.

CREATE TABLE paths.node (
    id           bigserial PRIMARY KEY,
    geom         geometry(Point, 25833) NOT NULL
);
CREATE INDEX node_geom_gix ON paths.node USING GIST (geom);

-- Source enum: track where each edge originated so re-ingest can
-- safely soft-delete missing rows scoped to one source.
CREATE TYPE paths.ingest_source AS ENUM ('fkb', 'turbase', 'dnt', 'manual');

CREATE TABLE paths.edge (
    id                 bigserial PRIMARY KEY,
    source_node        bigint,
    target_node        bigint,
    geom               geometry(LineString, 25833) NOT NULL,
    length_m           double precision GENERATED ALWAYS AS (ST_Length(geom)) STORED,
    elevation_gain_m   double precision,
    elevation_loss_m   double precision,
    fkb_type           text NOT NULL,
    marking            text,
    surface            text,
    season             text[] NOT NULL DEFAULT ARRAY['summer'],
    attrs              jsonb NOT NULL DEFAULT '{}'::jsonb,
    attr_hash          text NOT NULL,
    ingest_source      paths.ingest_source NOT NULL DEFAULT 'fkb',
    ingested_at        timestamptz NOT NULL DEFAULT now(),
    deleted_at         timestamptz
);

CREATE INDEX edge_geom_gix          ON paths.edge USING GIST (geom);
CREATE INDEX edge_fkb_type_idx      ON paths.edge (fkb_type) WHERE deleted_at IS NULL;
CREATE INDEX edge_source_node_idx   ON paths.edge (source_node) WHERE deleted_at IS NULL;
CREATE INDEX edge_target_node_idx   ON paths.edge (target_node) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX edge_attr_hash_uq ON paths.edge (attr_hash) WHERE deleted_at IS NULL;

-- Staging tables per ingest job. Truncate-and-load: every run wipes
-- and refills, then the upsert step diffs against `paths.edge`.
CREATE TABLE paths.staging_fkb_sti (
    geom        geometry(LineString, 25833) NOT NULL,
    fkb_type    text NOT NULL,
    marking     text,
    surface     text,
    attrs       jsonb NOT NULL DEFAULT '{}'::jsonb,
    attr_hash   text NOT NULL
);
CREATE INDEX staging_fkb_sti_hash_idx ON paths.staging_fkb_sti (attr_hash);
