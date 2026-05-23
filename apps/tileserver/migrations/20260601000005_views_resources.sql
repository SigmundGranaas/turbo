-- Per-resource views: filtered subset of `paths.edge` UNION'd with the
-- matching `paths.curated_route` rows (published only).
--
-- The shape is uniform across resources so MVT/GeoJSON queries can
-- treat them interchangeably: (id, geom, name, difficulty, length_m,
-- elevation_gain_m, elevation_loss_m, marking, surface, season,
-- description, source, attribution).
--
-- Edge rows synthesise null-filled metadata; only curated rows carry
-- name/difficulty/description.

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
      AND e.fkb_type IN ('sti', 'traktorveg')
UNION ALL
    SELECT
        ('route:' || r.id::text)                              AS id,
        ST_GeometryN(r.geom, 1)                               AS geom,
        r.name                                                AS name,
        r.description                                         AS description,
        r.difficulty                                          AS difficulty,
        r.length_m,
        r.elevation_gain_m,
        r.elevation_loss_m,
        r.marking,
        r.surface,
        r.season,
        r.source                                              AS source,
        COALESCE(r.attribution, '© Kartverket, Nasjonal Turbase') AS attribution
    FROM paths.curated_route r
    WHERE r.resource = 'hiking-trails'
      AND r.status = 'published';

CREATE OR REPLACE VIEW paths.v_ski_tracks AS
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
      AND e.fkb_type = 'skiloype'
UNION ALL
    SELECT
        ('route:' || r.id::text)                              AS id,
        ST_GeometryN(r.geom, 1)                               AS geom,
        r.name,
        r.description,
        r.difficulty,
        r.length_m,
        r.elevation_gain_m,
        r.elevation_loss_m,
        r.marking,
        r.surface,
        r.season,
        r.source,
        COALESCE(r.attribution, '© Kartverket, Skisporet.no') AS attribution
    FROM paths.curated_route r
    WHERE r.resource = 'ski-tracks' AND r.status = 'published';

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
      AND e.fkb_type IN ('skogsbilveg', 'traktorveg')
UNION ALL
    SELECT
        ('route:' || r.id::text)                              AS id,
        ST_GeometryN(r.geom, 1)                               AS geom,
        r.name,
        r.description,
        r.difficulty,
        r.length_m,
        r.elevation_gain_m,
        r.elevation_loss_m,
        r.marking,
        r.surface,
        r.season,
        r.source,
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
      AND e.fkb_type IN ('sykkelvei', 'skogsbilveg')
UNION ALL
    SELECT
        ('route:' || r.id::text)                              AS id,
        ST_GeometryN(r.geom, 1)                               AS geom,
        r.name,
        r.description,
        r.difficulty,
        r.length_m,
        r.elevation_gain_m,
        r.elevation_loss_m,
        r.marking,
        r.surface,
        r.season,
        r.source,
        COALESCE(r.attribution, '© Kartverket') AS attribution
    FROM paths.curated_route r
    WHERE r.resource = 'cycling-routes' AND r.status = 'published';
