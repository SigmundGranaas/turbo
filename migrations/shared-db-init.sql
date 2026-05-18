-- Initial database creation for the shared-Postgres deploy variant.
-- Runs once at first startup of the postgis/postgis:17 container; ignored
-- on subsequent runs (Postgres only executes /docker-entrypoint-initdb.d
-- scripts when the data directory is empty).
--
-- Flyway then populates each database from src/{Auth,Geo,Activity}/db/migrations.

CREATE DATABASE auth;
CREATE DATABASE geo;
CREATE DATABASE activity;

-- PostGIS lives in the geo database only.
\connect geo;
CREATE EXTENSION IF NOT EXISTS postgis;
