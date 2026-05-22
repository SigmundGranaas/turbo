-- Curated routes: named, attributed multi-segment routes. Authored
-- either by ingest (turbase, dnt) or by humans via the admin panel
-- (`source = 'manual'`, `source = 'gpx-import'`).
--
-- Storage is denormalised: `geom` is the source of truth at render
-- time, `edge_ids` carries traceability into the routable graph. When
-- FKB re-ingest touches an edge referenced here, the diff stage sets
-- `needs_review = true` and surfaces it in the admin Jobs screen.

CREATE TYPE paths.route_status AS ENUM ('draft', 'published', 'archived');

CREATE TABLE paths.curated_route (
    id                 uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    external_id        text,
    resource           text NOT NULL CHECK (resource IN (
                          'hiking-trails', 'ski-tracks',
                          'forest-roads', 'cycling-routes'
                      )),
    slug               text NOT NULL,
    name               text,
    description        text,
    difficulty         text,
    season             text[] NOT NULL DEFAULT ARRAY['summer'],
    marking            text,
    surface            text,
    edge_ids           bigint[] NOT NULL DEFAULT ARRAY[]::bigint[],
    geom               geometry(MultiLineString, 25833) NOT NULL,
    length_m           double precision GENERATED ALWAYS AS (ST_Length(geom)) STORED,
    elevation_gain_m   double precision,
    elevation_loss_m   double precision,
    source             text NOT NULL DEFAULT 'manual',
    status             paths.route_status NOT NULL DEFAULT 'draft',
    attribution        text,
    created_by         uuid,
    created_at         timestamptz NOT NULL DEFAULT now(),
    updated_at         timestamptz NOT NULL DEFAULT now(),
    needs_review       boolean NOT NULL DEFAULT false,
    UNIQUE (resource, slug)
);

CREATE INDEX curated_route_geom_gix     ON paths.curated_route USING GIST (geom);
CREATE INDEX curated_route_resource_idx ON paths.curated_route (resource, status);
CREATE INDEX curated_route_name_trgm    ON paths.curated_route USING GIN (name gin_trgm_ops);

CREATE OR REPLACE FUNCTION paths.set_updated_at() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END $$;

CREATE TRIGGER curated_route_updated_at
    BEFORE UPDATE ON paths.curated_route
    FOR EACH ROW EXECUTE FUNCTION paths.set_updated_at();
