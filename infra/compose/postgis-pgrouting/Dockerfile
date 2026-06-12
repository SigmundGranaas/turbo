# PostGIS + pgRouting on top of the official postgis/postgis image. The
# tileserver migrations call CREATE EXTENSION pgrouting; the upstream
# postgis image doesn't ship it so we layer the Debian package on top.
#
# Pin a STABLE PostGIS release tag, not the rolling `:17-master` dev tag.
# `:17-master` ships a PostGIS built from git whose SQL/C library can drift
# out of sync — it currently fails `CREATE EXTENSION postgis` with
# `could not find function "ST_MMin"`, which breaks every migration at boot.
# `17-3.5` is the stable PG17 + PostGIS 3.5 image.

FROM postgis/postgis:17-3.5

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        postgresql-17-pgrouting \
    && rm -rf /var/lib/apt/lists/*
