-- Synthetic N50 Kartdata mini-dump.
-- Mirrors the schema shape Kartverket emits (hash-named schema, same
-- table+column names) so the same n50 ingest code that handles the
-- 25 GB real dump works against this 5 KB test fixture.
--
-- Coordinates: Sognsvann area (~ 595000, 6650000 in EPSG:25833) so
-- everything is close to where the existing recommend-seed fixture
-- puts its graph nodes.

CREATE SCHEMA IF NOT EXISTS n50kartdata_aabbccddeeff11223344556677889900;

CREATE TABLE n50kartdata_aabbccddeeff11223344556677889900.innsjo (
    objid integer NOT NULL,
    objtype text,
    omrade public.geometry(Geometry, 25833),
    vatnlopenummer integer,
    hoyde integer,
    oppdateringsdato date
);
INSERT INTO n50kartdata_aabbccddeeff11223344556677889900.innsjo (objid, objtype, omrade, vatnlopenummer, hoyde, oppdateringsdato) VALUES
  (1001, 'Innsjø',
   ST_GeomFromText('POLYGON((595100 6650500, 595300 6650500, 595300 6650700, 595100 6650700, 595100 6650500))', 25833),
   12345, 178, '2026-01-01'),
  (1002, 'Innsjø',
   ST_GeomFromText('POLYGON((596000 6651000, 596200 6651000, 596200 6651200, 596000 6651200, 596000 6651000))', 25833),
   23456, 220, '2026-01-01');

CREATE TABLE n50kartdata_aabbccddeeff11223344556677889900.havflate (
    objid integer NOT NULL, objtype text,
    omrade public.geometry(Geometry, 25833), oppdateringsdato date
);
-- No sea polygons near Oslomarka; empty is the realistic case.

CREATE TABLE n50kartdata_aabbccddeeff11223344556677889900.snoisbre (
    objid integer NOT NULL, objtype text,
    omrade public.geometry(Geometry, 25833), oppdateringsdato date
);
INSERT INTO n50kartdata_aabbccddeeff11223344556677889900.snoisbre (objid, objtype, omrade, oppdateringsdato) VALUES
  (2001, 'Isbre',
   ST_GeomFromText('POLYGON((598000 6652000, 598200 6652000, 598200 6652200, 598000 6652200, 598000 6652000))', 25833),
   '2026-01-01');

CREATE TABLE n50kartdata_aabbccddeeff11223344556677889900.skog (
    objid integer NOT NULL, objtype text,
    omrade public.geometry(Geometry, 25833), oppdateringsdato date
);
INSERT INTO n50kartdata_aabbccddeeff11223344556677889900.skog (objid, objtype, omrade, oppdateringsdato) VALUES
  (3001, 'Skog',
   ST_GeomFromText('POLYGON((595000 6650000, 596000 6650000, 596000 6651000, 595000 6651000, 595000 6650000))', 25833),
   '2026-01-01');

CREATE TABLE n50kartdata_aabbccddeeff11223344556677889900.myr (
    objid integer NOT NULL, objtype text,
    omrade public.geometry(Geometry, 25833), oppdateringsdato date
);
INSERT INTO n50kartdata_aabbccddeeff11223344556677889900.myr (objid, objtype, omrade, oppdateringsdato) VALUES
  (4001, 'Myr',
   ST_GeomFromText('POLYGON((595500 6650500, 595700 6650500, 595700 6650700, 595500 6650700, 595500 6650500))', 25833),
   '2026-01-01');

CREATE TABLE n50kartdata_aabbccddeeff11223344556677889900.apentomrade (
    objid integer NOT NULL, objtype text,
    omrade public.geometry(Geometry, 25833), oppdateringsdato date
);
INSERT INTO n50kartdata_aabbccddeeff11223344556677889900.apentomrade (objid, objtype, omrade, oppdateringsdato) VALUES
  (5001, 'ÅpentOmrade',
   ST_GeomFromText('POLYGON((597000 6651000, 598000 6651000, 598000 6652000, 597000 6652000, 597000 6651000))', 25833),
   '2026-01-01');

CREATE TABLE n50kartdata_aabbccddeeff11223344556677889900.dyrketmark (
    objid integer NOT NULL, objtype text,
    omrade public.geometry(Geometry, 25833), oppdateringsdato date
);
-- No farmland in Oslomarka in the test fixture; empty is fine.

-- Regulated lakes/reservoirs + intermittent freshwater. The vann upsert
-- reads both (added when regulated lakes + river-area polygons were folded
-- into terrain.water_polygon). Kept empty here so the canonical water count
-- stays at the 2 natural lakes the vann test asserts — the tables just need
-- to EXIST so the upsert SQL resolves against the staging schema.
CREATE TABLE n50kartdata_aabbccddeeff11223344556677889900.innsjoregulert (
    objid integer NOT NULL,
    objtype text,
    omrade public.geometry(Geometry, 25833),
    vatnlopenummer integer,
    lavesteregulertevannstand integer,
    hoyde integer,
    oppdateringsdato date
);

CREATE TABLE n50kartdata_aabbccddeeff11223344556677889900.ferskvanntorrfall (
    objid integer NOT NULL,
    objtype text,
    omrade public.geometry(Geometry, 25833),
    oppdateringsdato date
);

CREATE TABLE n50kartdata_aabbccddeeff11223344556677889900.elv (
    objid integer NOT NULL, objtype text,
    omrade public.geometry(Geometry, 25833), oppdateringsdato date
);

CREATE TABLE n50kartdata_aabbccddeeff11223344556677889900.stedsnavntekst (
    objid integer NOT NULL,
    objtype text,
    stedsnavnnummer integer,
    stedsnummer integer,
    skrivematenummer integer,
    geometri public.geometry(Geometry, 25833),
    streng text,
    fulltekst text,
    navneobjekttype text,
    navneobjektgruppe text,
    oppdateringsdato date
);
INSERT INTO n50kartdata_aabbccddeeff11223344556677889900.stedsnavntekst
  (objid, objtype, geometri, streng, fulltekst, navneobjekttype, navneobjektgruppe) VALUES
  (6001, 'Stedsnavn',
   ST_GeomFromText('POINT(596800 6651800)', 25833),
   'Vettakollen', 'Vettakollen', 'Fjelltopp', 'Fjell'),
  (6002, 'Stedsnavn',
   ST_GeomFromText('POINT(596000 6651400)', 25833),
   'Tryvannshogda', 'Tryvannshogda', 'Topp', 'Fjell'),
  (6003, 'Stedsnavn',
   ST_GeomFromText('POINT(595200 6650600)', 25833),
   'Sognsvann', 'Sognsvann', 'Innsjø', 'Vann'),
  (6004, 'Stedsnavn',
   ST_GeomFromText('POINT(596400 6651200)', 25833),
   'Kobberhaughytta', 'Kobberhaughytta', 'Turisthytte', 'Bebyggelse'),
  (6005, 'Stedsnavn',
   ST_GeomFromText('POINT(595600 6651000)', 25833),
   'Frognerseteren', 'Frognerseteren', 'Bebyggelse', 'Bebyggelse');

CREATE TABLE n50kartdata_aabbccddeeff11223344556677889900.terrengpunkt (
    objid integer NOT NULL,
    objtype text,
    posisjon public.geometry(Geometry, 25833),
    hoyde integer,
    datafangstdato date,
    oppdateringsdato date,
    medium text,
    malemetode text,
    noyaktighet text
);
INSERT INTO n50kartdata_aabbccddeeff11223344556677889900.terrengpunkt
  (objid, objtype, posisjon, hoyde) VALUES
  (7001, 'Terrengpunkt', ST_GeomFromText('POINT(597500 6651500)', 25833), 720),  -- summit candidate
  (7002, 'Terrengpunkt', ST_GeomFromText('POINT(595800 6650200)', 25833), 250);  -- low; filtered out

CREATE TABLE n50kartdata_aabbccddeeff11223344556677889900.veglenke (
    objid integer NOT NULL,
    objtype text,
    medium text,
    datafangstdato date,
    oppdateringsdato date,
    malemetode text,
    noyaktighet text,
    senterlinje public.geometry(Geometry, 25833),
    typeveg text,
    vegkategori text,
    vegfase text,
    vegnummer integer,
    motorvegtype text,
    rutemerking text,
    vedlikeholds text
);
INSERT INTO n50kartdata_aabbccddeeff11223344556677889900.veglenke
  (objid, objtype, senterlinje, typeveg, vegkategori) VALUES
  (8001, 'Veglenke',
   ST_GeomFromText('LINESTRING(595000 6650000, 595200 6650000, 595400 6650100)', 25833),
   'Skogsbilveg', 'Privat'),
  (8002, 'Veglenke',
   ST_GeomFromText('LINESTRING(596400 6651200, 596800 6651800)', 25833),
   'Traktorveg', 'Privat');

-- Lookup table N50 ships but we don't actually need at runtime.
CREATE TABLE n50kartdata_aabbccddeeff11223344556677889900.typeveg (
    identifier text, description text
);

-- BygningerOgAnlegg theme: building footprints. The bygning upsert routes
-- these to terrain.building_polygon. Two footprints near Sognsvann; one
-- carries a name (a cabin), one does not.
CREATE TABLE n50kartdata_aabbccddeeff11223344556677889900.bygning_omrade (
    objid integer NOT NULL,
    objtype text,
    datafangstdato date,
    oppdateringsdato date,
    malemetode text,
    noyaktighet text,
    omrade public.geometry(Geometry, 25833),
    bygningstype text,
    betjeningsgrad text,
    hytteeier text,
    tilgjengelighet text,
    navn text
);
INSERT INTO n50kartdata_aabbccddeeff11223344556677889900.bygning_omrade
  (objid, objtype, omrade, bygningstype, navn) VALUES
  (9001, 'Bygning',
   ST_GeomFromText('POLYGON((595400 6650400, 595420 6650400, 595420 6650420, 595400 6650420, 595400 6650400))', 25833),
   '161', 'Kobberhaughytta'),
  (9002, 'Bygning',
   ST_GeomFromText('POLYGON((596100 6651100, 596115 6651100, 596115 6651112, 596100 6651112, 596100 6651100))', 25833),
   '111', NULL);

-- Høyde theme: contour lines. Three object types share one shape; the
-- upsert routes them to terrain.contour as main/auxiliary/depression and
-- flags is_index on the 100 m lines. Geometry near Sognsvann to match the
-- rest of the fixture. hoyde values chosen to exercise index detection:
-- 200 + 600 are index (mod 100 = 0); 220 is a plain main line.
CREATE TABLE n50kartdata_aabbccddeeff11223344556677889900.hoydekurve (
    objid integer NOT NULL,
    objtype text,
    senterlinje public.geometry(Geometry, 25833),
    hoyde integer,
    datafangstdato date,
    oppdateringsdato date,
    medium text,
    malemetode text,
    noyaktighet text
);
INSERT INTO n50kartdata_aabbccddeeff11223344556677889900.hoydekurve
  (objid, objtype, senterlinje, hoyde, medium) VALUES
  (8101, 'Høydekurve', ST_GeomFromText('LINESTRING(595000 6650000, 595300 6650100, 595600 6650050)', 25833), 200, 'T'),
  (8102, 'Høydekurve', ST_GeomFromText('LINESTRING(595100 6650300, 595400 6650400, 595700 6650350)', 25833), 220, 'T'),
  (8103, 'Høydekurve', ST_GeomFromText('LINESTRING(597000 6651000, 597300 6651200, 597600 6651100)', 25833), 600, 'T');

CREATE TABLE n50kartdata_aabbccddeeff11223344556677889900.hjelpekurve (
    objid integer NOT NULL,
    objtype text,
    senterlinje public.geometry(Geometry, 25833),
    hoyde integer,
    datafangstdato date,
    oppdateringsdato date,
    medium text,
    malemetode text,
    noyaktighet text
);
INSERT INTO n50kartdata_aabbccddeeff11223344556677889900.hjelpekurve
  (objid, objtype, senterlinje, hoyde, medium) VALUES
  (8201, 'Hjelpekurve', ST_GeomFromText('LINESTRING(595200 6650500, 595500 6650600)', 25833), 210, 'T');

CREATE TABLE n50kartdata_aabbccddeeff11223344556677889900.forsenkningskurve (
    objid integer NOT NULL,
    objtype text,
    senterlinje public.geometry(Geometry, 25833),
    hoyde integer,
    datafangstdato date,
    oppdateringsdato date,
    medium text,
    malemetode text,
    noyaktighet text
);
INSERT INTO n50kartdata_aabbccddeeff11223344556677889900.forsenkningskurve
  (objid, objtype, senterlinje, hoyde, medium) VALUES
  (8301, 'Forsenkningskurve', ST_GeomFromText('LINESTRING(596000 6650800, 596100 6650900, 596000 6650800)', 25833), 180, 'T');

