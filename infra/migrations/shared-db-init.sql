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

-- Activities module: one cross-kind summary store + one per activity kind.
CREATE DATABASE activities;
CREATE DATABASE fishing;
CREATE DATABASE backcountry_ski;
CREATE DATABASE hiking;
CREATE DATABASE xc_ski;
CREATE DATABASE packrafting;
CREATE DATABASE freediving;

-- PostGIS extension on every spatial database.
\connect geo;
CREATE EXTENSION IF NOT EXISTS postgis;

\connect tracks;
CREATE EXTENSION IF NOT EXISTS postgis;

\connect activities;
CREATE EXTENSION IF NOT EXISTS postgis;

\connect fishing;
CREATE EXTENSION IF NOT EXISTS postgis;

\connect backcountry_ski;
CREATE EXTENSION IF NOT EXISTS postgis;

\connect hiking;
CREATE EXTENSION IF NOT EXISTS postgis;

\connect xc_ski;
CREATE EXTENSION IF NOT EXISTS postgis;

\connect packrafting;
CREATE EXTENSION IF NOT EXISTS postgis;

\connect freediving;
CREATE EXTENSION IF NOT EXISTS postgis;
