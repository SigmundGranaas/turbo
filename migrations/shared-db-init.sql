-- Initial database creation for the shared-Postgres deploy variant.
-- Runs once at first startup of the postgis/postgis:17 container; ignored
-- on subsequent runs (Postgres only executes /docker-entrypoint-initdb.d
-- scripts when the data directory is empty).
--
-- EF Core then creates the schema for each database in-process at host
-- startup (see MigrateModuleDatabaseAsync in src/Shared/Turbo.Hosting.Postgres).

CREATE DATABASE auth;
CREATE DATABASE geo;
CREATE DATABASE tracks;
CREATE DATABASE collections;

-- PostGIS extension on the two spatial databases.
\connect geo;
CREATE EXTENSION IF NOT EXISTS postgis;

\connect tracks;
CREATE EXTENSION IF NOT EXISTS postgis;
