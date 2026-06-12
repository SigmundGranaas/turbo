-- Reconcile the fkb_type vocabulary across producers and consumers.
--
-- Two spellings were in use: the N50 vegnett upsert and the graph encoder
-- (`encode_fkb_type`) emit/understand the "vei" forms (`traktorvei`,
-- `skogsvei`), while these resource views — and the FKB WFS ingest — used the
-- "veg" forms (`traktorveg`, `skogsbilveg`). The net effect: N50 vegnett edges
-- never surfaced in forest-roads / hiking / cycling layers, and FKB road edges
-- encoded as 0 (unknown) in the routing graph.
--
-- Canonical = the encoder's "vei" vocabulary: {sti, vei, traktorvei, skogsvei,
-- sykkelvei, skiloype}. FKB ingest is normalised to it (fkb_wfs.rs) and these
-- views are updated to match. Only the three road/trail views change; ski
-- tracks already keyed on the canonical `skiloype`.

CREATE OR REPLACE VIEW paths.v_hiking_trails AS
    SELECT
        ('edge:' || e.id)::text                              AS id,
        e.geom                                                AS geom,
        NULL::text                                            AS name,
        NULL::text                                            AS description,
        NULL::text                                            AS difficulty,
        e.length_m,
        e.elevation_gain_m,
        e.elevation_loss_m,
        e.marking,
        e.surface,
        e.season,
        'fkb'::text                                           AS source,
        '© Kartverket'::text                                  AS attribution
    FROM paths.edge e
    WHERE e.deleted_at IS NULL
      AND e.fkb_type IN ('sti', 'traktorvei')
UNION ALL
    SELECT
        ('route:' || r.id::text)                              AS id,
        ST_GeometryN(r.geom, 1)                               AS geom,
        r.name, r.description, r.difficulty,
        r.length_m, r.elevation_gain_m, r.elevation_loss_m,
        r.marking, r.surface, r.season, r.source,
        COALESCE(r.attribution, '© Kartverket, Nasjonal Turbase') AS attribution
    FROM paths.curated_route r
    WHERE r.resource = 'hiking-trails'
      AND r.status = 'published';

CREATE OR REPLACE VIEW paths.v_forest_roads AS
    SELECT
        ('edge:' || e.id)::text                              AS id,
        e.geom                                                AS geom,
        NULL::text                                            AS name,
        NULL::text                                            AS description,
        NULL::text                                            AS difficulty,
        e.length_m,
        e.elevation_gain_m,
        e.elevation_loss_m,
        e.marking,
        e.surface,
        e.season,
        'fkb'::text                                           AS source,
        '© Kartverket'::text                                  AS attribution
    FROM paths.edge e
    WHERE e.deleted_at IS NULL
      AND e.fkb_type IN ('skogsvei', 'traktorvei')
UNION ALL
    SELECT
        ('route:' || r.id::text)                              AS id,
        ST_GeometryN(r.geom, 1)                               AS geom,
        r.name, r.description, r.difficulty,
        r.length_m, r.elevation_gain_m, r.elevation_loss_m,
        r.marking, r.surface, r.season, r.source,
        COALESCE(r.attribution, '© Kartverket') AS attribution
    FROM paths.curated_route r
    WHERE r.resource = 'forest-roads' AND r.status = 'published';

CREATE OR REPLACE VIEW paths.v_cycling_routes AS
    SELECT
        ('edge:' || e.id)::text                              AS id,
        e.geom                                                AS geom,
        NULL::text                                            AS name,
        NULL::text                                            AS description,
        NULL::text                                            AS difficulty,
        e.length_m,
        e.elevation_gain_m,
        e.elevation_loss_m,
        e.marking,
        e.surface,
        e.season,
        'fkb'::text                                           AS source,
        '© Kartverket'::text                                  AS attribution
    FROM paths.edge e
    WHERE e.deleted_at IS NULL
      AND e.fkb_type IN ('sykkelvei', 'skogsvei')
UNION ALL
    SELECT
        ('route:' || r.id::text)                              AS id,
        ST_GeometryN(r.geom, 1)                               AS geom,
        r.name, r.description, r.difficulty,
        r.length_m, r.elevation_gain_m, r.elevation_loss_m,
        r.marking, r.surface, r.season, r.source,
        COALESCE(r.attribution, '© Kartverket') AS attribution
    FROM paths.curated_route r
    WHERE r.resource = 'cycling-routes' AND r.status = 'published';
