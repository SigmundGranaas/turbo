# Builds two artifacts from one Cargo workspace: the `tileserver` binary
# that hosts both `serve` and `ingest` subcommands. CI publishes one
# image; the k8s CronJob and the Deployment both use it with different
# entry arguments.

FROM rust:1.90-bookworm AS chef
RUN cargo install cargo-chef --locked --version 0.1.68
WORKDIR /build

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
# SQLX_OFFLINE=true keeps the build hermetic — we don't use compile-time
# query checking yet (M1 query is a format!() string), so this flag is
# a safety net for when it's introduced.
ENV SQLX_OFFLINE=true
RUN cargo build --release --bin tileserver

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/tileserver /usr/local/bin/tileserver
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/tileserver"]
CMD ["serve"]
