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
