# PostGIS + pgRouting on top of the official postgis/postgis image. The
# tileserver migrations call CREATE EXTENSION pgrouting; the upstream
# postgis image doesn't ship it so we layer the Debian package on top.

FROM postgis/postgis:17-master

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        postgresql-17-pgrouting \
    && rm -rf /var/lib/apt/lists/*
