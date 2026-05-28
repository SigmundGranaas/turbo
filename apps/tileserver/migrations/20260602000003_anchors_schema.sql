-- anchors.*: places worth going to. Pre-snapped to the routing graph
-- at ingest time so target queries pay zero snap cost.
--
-- One row per real-world place; sources are merged into the `sources`
-- array column on conflict resolution at ingest.

CREATE SCHEMA IF NOT EXISTS anchors;

CREATE TABLE anchors.anchor (
    id                  bigserial PRIMARY KEY,
    kind                text NOT NULL,              -- summit|cabin|viewpoint|trailhead|parking|waterfeature|named_place
    geom                geometry(Point, 25833) NOT NULL,
    name                text,
    elevation_m         double precision,
    prominence_m        double precision,           -- summits only; from watershed walk at ingest
    snapped_node_id     bigint,                     -- paths.node.id, pre-snapped at ingest
    snap_distance_m     double precision,
    sources             text[] NOT NULL DEFAULT '{}',
    source_ref          text,
    attrs               jsonb NOT NULL DEFAULT '{}'::jsonb,
    ingested_at         timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX anchor_geom_gix          ON anchors.anchor USING GIST (geom);
CREATE INDEX anchor_kind_idx          ON anchors.anchor (kind);
CREATE INDEX anchor_snapped_node_idx  ON anchors.anchor (snapped_node_id);
CREATE INDEX anchor_prominence_idx    ON anchors.anchor (prominence_m DESC NULLS LAST)
    WHERE kind = 'summit';
CREATE UNIQUE INDEX anchor_source_ref_uq ON anchors.anchor (source_ref) WHERE source_ref IS NOT NULL;
