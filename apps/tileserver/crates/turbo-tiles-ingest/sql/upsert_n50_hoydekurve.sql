-- Upsert N50 "Høyde" theme contour lines → terrain.contour.
-- Sources after pgdump_load::restore: schema `n50_staging`.
--
-- Source table shape (identical across all three):
--   n50_staging.hoydekurve        (objid, objtype, senterlinje GEOMETRY(_,25833),
--                                  hoyde int, medium, malemetode, noyaktighet, ...)
--   n50_staging.hjelpekurve       (same shape)
--   n50_staging.forsenkningskurve (same shape)
--
-- Geometry column is `senterlinje` (the contour centreline), `hoyde` is the
-- elevation in whole metres. N50's base equidistance is 20 m; the bold
-- INDEX contour falls every 100 m (`hoyde % 100 = 0`).
--
-- senterlinje is typed GEOMETRY (not LineString) — most rows are simple
-- LineStrings but ST_Dump explodes any MultiLineString parts so nothing is
-- silently dropped and the canonical column stays a clean LineString.

DELETE FROM terrain.contour WHERE source = 'n50';

-- Main contours (Høydekurve).
INSERT INTO terrain.contour (geom, elev_m, kind, is_index, source, attrs)
SELECT
    (ST_Dump(s.senterlinje)).geom::geometry(LineString, 25833),
    s.hoyde,
    'main',
    (s.hoyde % 100 = 0),
    'n50',
    jsonb_build_object('objtype', s.objtype, 'medium', s.medium)
FROM n50_staging.hoydekurve s
WHERE s.senterlinje IS NOT NULL
  AND NOT ST_IsEmpty(s.senterlinje)
  AND s.hoyde IS NOT NULL;

-- Auxiliary helper contours (Hjelpekurve) — never index lines.
INSERT INTO terrain.contour (geom, elev_m, kind, is_index, source, attrs)
SELECT
    (ST_Dump(s.senterlinje)).geom::geometry(LineString, 25833),
    s.hoyde,
    'auxiliary',
    false,
    'n50',
    jsonb_build_object('objtype', s.objtype, 'medium', s.medium)
FROM n50_staging.hjelpekurve s
WHERE s.senterlinje IS NOT NULL
  AND NOT ST_IsEmpty(s.senterlinje)
  AND s.hoyde IS NOT NULL;

-- Depression contours (Forsenkningskurve) — closed contours around a
-- local minimum; styled with hachures downstream.
INSERT INTO terrain.contour (geom, elev_m, kind, is_index, source, attrs)
SELECT
    (ST_Dump(s.senterlinje)).geom::geometry(LineString, 25833),
    s.hoyde,
    'depression',
    (s.hoyde % 100 = 0),
    'n50',
    jsonb_build_object('objtype', s.objtype, 'medium', s.medium)
FROM n50_staging.forsenkningskurve s
WHERE s.senterlinje IS NOT NULL
  AND NOT ST_IsEmpty(s.senterlinje)
  AND s.hoyde IS NOT NULL;
