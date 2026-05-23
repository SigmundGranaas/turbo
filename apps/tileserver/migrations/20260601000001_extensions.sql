-- PostGIS is provided by the container image; pgRouting needs to be
-- installed separately (see infra/compose/postgis-pgrouting/Dockerfile).
-- pg_trgm powers admin-panel search.
CREATE EXTENSION IF NOT EXISTS postgis;
CREATE EXTENSION IF NOT EXISTS pgrouting;
CREATE EXTENSION IF NOT EXISTS pg_trgm;

CREATE SCHEMA IF NOT EXISTS paths;
