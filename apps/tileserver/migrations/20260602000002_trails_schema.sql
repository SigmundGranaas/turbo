-- trails.*: curated trail network as a view over paths.edge.
-- The trail is a named, graded route composed of one or more
-- paths.edge rows (typically many — a 12 km marked trail can be
-- 200+ edges after FKB segmentation).

CREATE SCHEMA IF NOT EXISTS trails;

CREATE TABLE trails.trail (
    id              bigserial PRIMARY KEY,
    name            text,
    operator        text,                           -- 'DNT', 'STF', 'kommune', ...
    mark_colour     text,                           -- 'red', 'blue', 'black', 'T'
    grade           text,                           -- DNT: 'gronn'|'bla'|'rod'|'svart'
    season          text[] NOT NULL DEFAULT ARRAY['summer'],
    source          text NOT NULL,                  -- 'turbase'|'dnt'|'manual'
    source_ref      text,                           -- upstream id for re-ingest
    attrs           jsonb NOT NULL DEFAULT '{}'::jsonb,
    ingested_at     timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX trail_grade_idx       ON trails.trail (grade);
CREATE INDEX trail_operator_idx    ON trails.trail (operator);
CREATE INDEX trail_source_ref_idx  ON trails.trail (source, source_ref);

-- Edge membership: which paths.edge rows belong to which trail.
-- ON DELETE CASCADE on the trail side means re-ingesting a trail
-- can DELETE the row and reinsert without orphaning edge refs.
CREATE TABLE trails.trail_edge (
    trail_id        bigint NOT NULL REFERENCES trails.trail(id) ON DELETE CASCADE,
    edge_id         bigint NOT NULL,
    seq             integer NOT NULL,
    PRIMARY KEY (trail_id, edge_id)
);
CREATE INDEX trail_edge_edge_idx ON trails.trail_edge (edge_id);
