-- Initial database creation for the shared-Postgres deploy variant.
-- Runs once at first startup of the postgis/postgis:17 container; ignored
-- on subsequent runs (Postgres only executes /docker-entrypoint-initdb.d
-- scripts when the data directory is empty).
--
-- One database per service. The activities service is itself modular
-- (fishing, hiking, …) but all its kinds share one database and isolate
-- themselves with Postgres schemas owned internally by each module.
-- EF Core creates each module's schema + tables in-process at host
-- startup (see MigrateModuleDatabaseAsync in src/Shared/Turbo.Hosting.Postgres).

CREATE DATABASE auth;
CREATE DATABASE sharing;
CREATE DATABASE geo;
CREATE DATABASE tracks;
CREATE DATABASE collections;
CREATE DATABASE activities;

-- PostGIS extension on every spatial database.
\connect geo;
CREATE EXTENSION IF NOT EXISTS postgis;

\connect tracks;
CREATE EXTENSION IF NOT EXISTS postgis;

\connect activities;
CREATE EXTENSION IF NOT EXISTS postgis;
