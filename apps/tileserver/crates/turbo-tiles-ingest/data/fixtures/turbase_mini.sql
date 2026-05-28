-- Synthetic Turrutebasen mini-dump matching the schema of the real
-- Friluftsliv PostGIS dump (hash-named schema, fotrute/skiloype/etc.
-- table shapes). Used by integration tests in lieu of the 83 MB real
-- file so we can exercise the full turbase ingest + upsert in
-- seconds, not minutes.

CREATE SCHEMA IF NOT EXISTS turogfriluftsruter_aabbccddeeff11223344556677889900;

CREATE TABLE turogfriluftsruter_aabbccddeeff11223344556677889900.fotrute (
    objid integer NOT NULL,
    objtype text,
    skilting boolean,
    anleggsnummer text,
    uukoblingsid text,
    belysning boolean,
    senterlinje public.geometry(Geometry, 25833),
    lokalid text,
    navnerom text,
    versjonid text,
    datafangstdato date,
    oppdateringsdato timestamp with time zone,
    noyaktighet integer,
    opphav text,
    omradeid integer,
    originaldatavert text,
    kopidato timestamp with time zone,
    informasjon text,
    merking text,
    rutefolger text,
    underlagstype text,
    rutebredde text,
    trafikkbelastning text,
    sesong text,
    malemetode text
);
INSERT INTO turogfriluftsruter_aabbccddeeff11223344556677889900.fotrute
  (objid, objtype, senterlinje, lokalid, anleggsnummer, merking, sesong, rutefolger, rutebredde) VALUES
  (9001, 'Fotrute',
   ST_GeomFromText('LINESTRING(595200 6650200, 595400 6650400, 595600 6650600)', 25833),
   'localid-9001', 'A001', 'Rød', 'Sommer', 'Stiløype', '50 cm'),
  (9002, 'Fotrute',
   ST_GeomFromText('LINESTRING(595600 6650600, 595800 6650800, 596000 6651000)', 25833),
   'localid-9002', 'A001', 'Rød', 'Sommer', 'Stiløype', '50 cm'),
  (9003, 'Fotrute',
   ST_GeomFromText('LINESTRING(596000 6651000, 596200 6651200, 596400 6651400)', 25833),
   'localid-9003', 'A001', 'Rød', 'Sommer', 'Stiløype', '50 cm'),
  (9004, 'Fotrute',
   ST_GeomFromText('LINESTRING(596400 6651400, 596600 6651600, 596800 6651800)', 25833),
   'localid-9004', 'A002', 'Blå', 'Sommer', 'Stiløype', '50 cm');

CREATE TABLE turogfriluftsruter_aabbccddeeff11223344556677889900.skiloype (
    objid integer NOT NULL,
    objtype text,
    senterlinje public.geometry(Geometry, 25833),
    lokalid text,
    sesong text,
    rutebredde text
);
INSERT INTO turogfriluftsruter_aabbccddeeff11223344556677889900.skiloype
  (objid, objtype, senterlinje, lokalid, sesong) VALUES
  (10001, 'Skiløype',
   ST_GeomFromText('LINESTRING(595800 6650800, 596200 6651200)', 25833),
   'localid-10001', 'Vinter');
