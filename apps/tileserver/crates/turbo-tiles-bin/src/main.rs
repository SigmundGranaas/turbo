use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

mod eval_terrain;
mod routing_setup;

// Heap profiler (opt-in via `--features dhat-heap`). The global
// allocator shim records every allocation; the `Profiler` guard created
// in `main` dumps `dhat-heap.json` + peak/total-bytes stats on exit.
#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

use tower_http::services::ServeDir;
use turbo_tiles_admin::AdminState;
use turbo_tiles_api::ApiState;
use turbo_tiles_auth::{AuthConfig, AuthState};
use turbo_tiles_db::{migrations, DbConfig};

#[derive(Debug, Parser)]
#[command(
    name = "tileserver",
    version,
    about = "Turbo curated paths tile + admin server"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the HTTP server (public /v1 + admin /admin).
    Serve {
        #[arg(long, env = "BIND", default_value = "0.0.0.0:8080")]
        bind: String,
        #[arg(long, env = "PUBLIC_BASE_URL", default_value = "http://localhost:8080")]
        public_base_url: String,
        /// Run pending migrations before starting. Default true in dev.
        #[arg(long, env = "AUTO_MIGRATE", default_value_t = true)]
        auto_migrate: bool,
        /// Directory containing primitive artifacts. Boot loads
        /// whatever is present and leaves the rest in degraded mode
        /// (endpoints return 503 until rebuilt).
        #[arg(long, env = "TILESERVER_ARTIFACT_DIR")]
        artifacts_dir: Option<std::path::PathBuf>,
        /// Skip the Postgres connection entirely. Catalog/resource/
        /// tile endpoints become 503; primitive endpoints are
        /// unaffected. Useful for the "artifact-only" production
        /// deployment described in the architecture doc.
        #[arg(long, env = "NO_DB", default_value_t = false)]
        no_db: bool,
    },
    /// Open each present primitive artifact, run a sanity probe, and
    /// report pass/fail. Doesn't bind a socket.
    VerifyArtifacts {
        #[arg(long, env = "TILESERVER_ARTIFACT_DIR")]
        dir: std::path::PathBuf,
        /// Optional directory containing baseline `*.health.json`
        /// sidecars to diff against. Any stat that drifted by more
        /// than `--baseline-pct` (default 10%) or any newly-appeared
        /// warning is promoted to the top-level output so the
        /// operator notices it before shipping.
        #[arg(long)]
        baseline: Option<std::path::PathBuf>,
        /// Drift threshold for `--baseline`, expressed as percent of
        /// the baseline value. Below this magnitude the change is
        /// considered noise.
        #[arg(long, default_value_t = 10.0)]
        baseline_pct: f64,
    },
    /// Build one or all primitive artifacts from Postgres staging.
    BuildArtifacts {
        /// What to build. One of: dem, mask, graph, search, all.
        #[arg(long, default_value = "dem")]
        kind: String,
        /// Output directory. Each artifact lands under its canonical
        /// filename (e.g. `norway.dem`).
        #[arg(long, env = "TILESERVER_ARTIFACT_DIR")]
        out: std::path::PathBuf,
    },
    /// Run a one-shot ingest job.
    Ingest {
        #[arg(long)]
        job: String,
        /// Optional W,S,E,N bbox (lon/lat) for jobs that pull by area.
        /// `fkb-sti` defaults to a small Oslo-area window when this is
        /// omitted so demo runs stay quick.
        #[arg(long)]
        bbox: Option<String>,
        /// Filesystem path for bulk-file jobs (`dtm-load`).
        #[arg(long)]
        file: Option<std::path::PathBuf>,
        /// Geonorge area for `geonorge-fetch` / `provision-n50`: a two-digit
        /// county code (e.g. `03`) or `national`.
        #[arg(long)]
        area: Option<String>,
        /// Source label stamped on loaded raster rows (e.g. `dtm10`).
        #[arg(long, default_value = "dtm10")]
        source: String,
        /// For `dtm10-attach`: re-attach elevation to every edge,
        /// even those that already have a value. Used when a
        /// higher-resolution DEM is loaded after the initial pass.
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    /// Apply pending migrations and exit.
    Migrate,
    /// Headless terrain-corpus evaluation. Loads the production routing
    /// artifacts + layer stack in-process (no server, no DB), solves
    /// every ground-truth hike with force-off-trail prefs, and writes
    /// machine-readable per-hike JSON + a run summary (latency,
    /// per-route geometry hash). The autonomous routing dev loop.
    EvalTerrain {
        /// Corpus TOML of ground-truth hikes.
        #[arg(long, default_value = "tools/terrain-corpus.toml")]
        corpus: std::path::PathBuf,
        /// Directory containing the routing artifacts (norway.*).
        #[arg(long, env = "TILESERVER_ARTIFACT_DIR")]
        artifacts_dir: Option<std::path::PathBuf>,
        /// Output directory for per-hike JSON + `_summary.json`.
        #[arg(long, default_value = "/tmp/turbo-routing-eval")]
        out: std::path::PathBuf,
        /// Only evaluate hikes whose region or id contains this string.
        #[arg(long)]
        filter: Option<String>,
        /// Evaluate at most this many hikes (after filtering).
        #[arg(long)]
        limit: Option<usize>,
        /// Solve the corpus twice and fail if any route's geometry hash
        /// differs — catches accidental nondeterminism.
        #[arg(long, default_value_t = false)]
        check_determinism: bool,
        /// Which router to exercise: `off-trail` (force-off-trail FMM —
        /// quality vs ground truth is meaningful) or `unified`
        /// (production-default prefs — the unified A* users hit;
        /// geometry/latency/DEM-work regression lane).
        #[arg(long, default_value = "off-trail")]
        mode: eval_terrain::EvalMode,
        /// JSON CostConfigPatch applied to every solve (knob sweeps),
        /// e.g. '{"grade_limited_max_grade_deg": 18.0}'.
        #[arg(long)]
        override_json: Option<String>,
    },
}

/// One entry in the shared ingestion catalog (`infra/k8s/base/ingest/catalog.toml`),
/// mounted read-only via the `ingest-catalog` ConfigMap. We only read the N50
/// metadata UUID here; everything else is documentary or Places-owned.
#[derive(serde::Deserialize)]
struct CatalogSource {
    id: String,
    metadata_uuid: Option<String>,
}

#[derive(serde::Deserialize)]
struct IngestCatalog {
    #[serde(default)]
    source: Vec<CatalogSource>,
}

/// Load the shared ingestion catalog (if mounted) and point the N50 dataset at
/// its metadata UUID. Best-effort: a missing/unparseable catalog leaves the
/// compiled-in default in place, so this never breaks a deploy. Path override
/// via `INGEST_CATALOG_PATH`.
fn load_ingest_catalog() {
    let path = std::env::var("INGEST_CATALOG_PATH")
        .unwrap_or_else(|_| "/etc/turbo/ingest/catalog.toml".to_string());
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return; // not mounted (dev/CI) — keep compiled-in defaults
    };
    match toml::from_str::<IngestCatalog>(&raw) {
        Ok(cat) => {
            if let Some(uuid) = cat
                .source
                .iter()
                .find(|s| s.id == "n50")
                .and_then(|s| s.metadata_uuid.clone())
            {
                turbo_tiles_ingest::geonorge::set_n50_metadata_uuid(&uuid);
                tracing::info!(%uuid, "ingest catalog: N50 metadata UUID set from {path}");
            }
        }
        Err(e) => tracing::warn!("ingest catalog at {path} is unparseable ({e}); using defaults"),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Held for the whole process; dumps the heap profile on drop.
    #[cfg(feature = "dhat-heap")]
    let _dhat = dhat::Profiler::new_heap();

    init_tracing();
    load_ingest_catalog();
    let cli = Cli::parse();

    match cli.command {
        Command::Serve {
            bind,
            public_base_url,
            auto_migrate,
            artifacts_dir,
            no_db,
        } => serve(bind, public_base_url, auto_migrate, artifacts_dir, no_db).await,
        Command::VerifyArtifacts {
            dir,
            baseline,
            baseline_pct,
        } => verify_artifacts(dir, baseline, baseline_pct).await,
        Command::Ingest {
            job,
            bbox,
            file,
            area,
            source,
            force,
        } => ingest(&job, bbox.as_deref(), file, area, source, force).await,
        Command::BuildArtifacts { kind, out } => build_artifacts(&kind, out).await,
        Command::Migrate => migrate().await,
        Command::EvalTerrain {
            corpus,
            artifacts_dir,
            out,
            filter,
            limit,
            check_determinism,
            mode,
            override_json,
        } => eval_terrain::run(
            corpus,
            artifacts_dir,
            out,
            filter,
            limit,
            check_determinism,
            mode,
            override_json,
        ),
    }
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,tower_http=debug,sqlx=warn"));
    // Logs go to stderr so subcommands (build-artifacts, verify-
    // artifacts) can emit machine-parseable JSON reports on stdout
    // without interleaving the tracing stream.
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .json()
        .with_target(true)
        .with_writer(std::io::stderr)
        .init();
}

async fn serve(
    bind: String,
    public_base_url: String,
    auto_migrate: bool,
    artifacts_dir: Option<std::path::PathBuf>,
    no_db: bool,
) -> Result<()> {
    let db = if no_db {
        // Use a junk URL that the lazy pool will only attempt to
        // resolve when a DB-touching endpoint is actually hit. The
        // standard Postgres deploys keep DATABASE_URL set, so we
        // fall back to its value when present, even with --no-db.
        let url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://nobody@127.0.0.1:1/none".to_string());
        let cfg = DbConfig {
            url,
            max_connections: 1,
            min_connections: 0,
            statement_timeout_ms: 1_000,
        };
        let pool = cfg.connect_lazy().context("preparing lazy DB pool")?;
        tracing::warn!(
            "--no-db: skipping DB connect and migrations; DB endpoints will 503 on demand"
        );
        pool
    } else {
        let db_cfg = DbConfig::from_env().context("DATABASE_URL must be set")?;
        let pool = db_cfg.connect().await.context("connecting to database")?;
        if auto_migrate {
            tracing::info!("running migrations");
            migrations::apply(&pool)
                .await
                .context("running migrations")?;
        }
        pool
    };

    // Auth is only needed for the /admin + debug surface. A missing
    // JWT_SECRET must NOT block boot — the public tiles / basemap / routing
    // endpoints serve fine without it. Run in public-only mode and warn.
    let auth = AuthConfig::from_env_lenient();
    if !auth.enabled {
        tracing::warn!(
            "JWT_SECRET not set: running in public-only mode. /admin and \
             auth-gated debug endpoints will reject all requests; tiles, \
             basemap, and routing endpoints are unaffected."
        );
    }
    let auth_state = AuthState(Arc::new(auth.clone()));

    // Primitive handles are loaded once at boot from the artifact
    // directory. Missing artifacts leave the corresponding endpoint
    // in 503-degraded mode rather than failing the whole start-up.
    // The artifact-open + pathfinder-assembly logic is shared with the
    // headless `eval-terrain` command via `routing_setup`, so the
    // autonomous evaluation loop routes through the IDENTICAL layer
    // stack the server serves.
    let mut api_state = ApiState::new(db.clone(), auth, public_base_url.clone());
    let art = routing_setup::load_routing_artifacts(artifacts_dir.as_deref());
    api_state.dem = art.dem.clone();
    api_state.mask = art.mask.clone();
    api_state.graph = art.graph.clone();
    api_state.search = art.search.clone();

    let cost_config = routing_setup::load_cost_config();
    tracing::info!(
        off_trail_base_foot = cost_config.off_trail_base.foot,
        proximity_bonus = cost_config.trail_proximity.bonus_at_zero,
        slope_refuse_cell = cost_config.slope_cell.refuse_above_deg,
        "loaded cost configuration"
    );
    let (pf, landcover) =
        routing_setup::build_pathfinder(artifacts_dir.as_deref(), &art, cost_config);
    for (name, mask) in landcover {
        api_state.landcover.insert(name, mask);
    }
    tracing::info!(
        graph_loaded = api_state.graph.is_some(),
        "pathfinder assembled"
    );
    api_state.pathfinder = Some(std::sync::Arc::new(pf));
    // Clone the rendered-MVT cache handle before `api_state` moves into the
    // router, so the (re)provision tasks below can invalidate it when the
    // underlying data changes (else tiles cached while the DB was empty/old
    // would keep being served after a provision).
    let mvt_cache = api_state.mvt_tiles.clone();
    // Same for the rendered-raster cache, invalidated on the same provisions.
    let raster_cache = api_state.raster_tiles.clone();
    // Clone the basemap-readiness flag too: `/v1/basemap` tiles 503 until it's
    // set, so a fresh deploy never serves cacheable empty tiles while the DB is
    // still provisioning. Set it now if the DB already has data (a restart over a
    // populated DB serves immediately); the boot-provision task sets it once an
    // empty DB finishes loading.
    let basemap_ready = api_state.basemap_ready.clone();
    {
        let ready = basemap_ready.clone();
        let probe_db = db.clone();
        tokio::spawn(async move {
            let n: i64 = sqlx::query_scalar("SELECT count(*) FROM terrain.water_polygon")
                .fetch_one(&probe_db)
                .await
                .unwrap_or(0);
            if n > 0 {
                ready.store(true, std::sync::atomic::Ordering::Relaxed);
                tracing::info!(rows = n, "basemap ready — serving /v1/basemap");
            } else {
                tracing::warn!("basemap empty at boot — /v1/basemap returns 503 until provisioned");
            }
        });
    }
    let api_router = turbo_tiles_api::router(api_state);

    let admin_state = AdminState {
        db: db.clone(),
        auth: auth_state,
    };
    let admin_router = turbo_tiles_admin::router(admin_state);

    // SPA static mount: the React build output lives at
    // /var/lib/tileserver/admin/dist (the Dockerfile copies it there).
    // Falls back to the local Vite build dir during dev so
    // `cargo run -- serve` works alongside `npm run build` in
    // apps/admin/.
    let spa_dir = std::env::var("ADMIN_SPA_DIR")
        .unwrap_or_else(|_| "/var/lib/tileserver/admin/dist".to_string());
    let spa_dir_fallback = std::path::Path::new("../admin/dist");
    let spa_path = if std::path::Path::new(&spa_dir).is_dir() {
        std::path::PathBuf::from(spa_dir)
    } else if spa_dir_fallback.is_dir() {
        spa_dir_fallback.to_path_buf()
    } else {
        std::path::PathBuf::from(&spa_dir)
    };
    tracing::info!(spa_dir = %spa_path.display(), "admin SPA mount");
    // SPA fallback: client-side routes like /admin/app/resources/X
    // don't exist as files on disk. ServeDir's `fallback` returns
    // index.html for any unmatched path so the React router can take
    // over after page load (deep links + reloads work).
    let spa_index = spa_path.join("index.html");
    let spa_service = ServeDir::new(&spa_path)
        .append_index_html_on_directories(true)
        .fallback(tower_http::services::ServeFile::new(spa_index));

    // SPA mount — `nest_service` serves the static SPA files. When
    // dev-auth is enabled we additionally apply a middleware layer
    // that auto-redirects unauthenticated browser hits to
    // `/admin/dev-login`, so opening
    // `http://localhost:8090/admin/app/plot` in a fresh tab works
    // without a manual stop at `/admin/dev-login`. The middleware is
    // *only* registered when TURBO_DEV_AUTH=1, so production is
    // untouched.
    let mut app = axum::Router::new()
        .merge(api_router)
        .nest("/admin", admin_router);
    if turbo_tiles_admin::routes::dev_login::enabled() {
        app = app.nest_service(
            "/admin/app",
            tower::ServiceBuilder::new()
                .layer(axum::middleware::from_fn(
                    turbo_tiles_admin::routes::dev_redirect::dev_auto_login,
                ))
                .service(spa_service),
        );
    } else {
        app = app.nest_service("/admin/app", spa_service);
    }
    let app = app;

    // Spawn the TUS abandoned-upload sweeper. Runs every hour;
    // deletes incomplete uploads after 2 days and completed uploads
    // after 7 (curator window for triggering ingest).
    turbo_tiles_admin::routes::tus::spawn_sweeper(std::time::Duration::from_secs(3600));

    // Zero-touch deploy: if TILESERVER_PROVISION_ON_BOOT is set to an area
    // (e.g. `national` or `03`) AND the basemap is empty, download + restore
    // + upsert N50 in the background so a fresh deploy self-populates with no
    // operator action. Off by default; only fires on an empty DB so restarts
    // don't re-provision. Re-provisioning on a cadence is a separate concern.
    if let Ok(area) = std::env::var("TILESERVER_PROVISION_ON_BOOT") {
        let provision_db = db.clone();
        let cache = mvt_cache.clone();
        let raster = raster_cache.clone();
        let ready = basemap_ready.clone();
        tokio::spawn(async move {
            maybe_provision_on_boot(provision_db, area, cache, raster, ready).await;
        });
    }

    // Hands-off freshness: when TILESERVER_PROVISION_REFRESH_SECS is set,
    // periodically re-provision whatever area is currently loaded. The
    // freshness skip makes each tick cheap (download + hash) when Kartverket
    // hasn't republished, and a full refresh when it has. Off by default.
    if let Ok(secs) = std::env::var("TILESERVER_PROVISION_REFRESH_SECS") {
        match secs.parse::<u64>() {
            Ok(s) if s >= 60 => {
                let refresh_db = db.clone();
                let cache = mvt_cache.clone();
                let raster = raster_cache.clone();
                tokio::spawn(async move {
                    refresh_loop(refresh_db, std::time::Duration::from_secs(s), cache, raster)
                        .await;
                });
            }
            _ => tracing::warn!(
                value = %secs,
                "TILESERVER_PROVISION_REFRESH_SECS must be an integer >= 60; refresh disabled"
            ),
        }
    }

    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .with_context(|| format!("binding {bind}"))?;
    tracing::info!(%bind, %public_base_url, "tileserver listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

/// Background boot-time provisioning. Runs the full N50 chain for `area`
/// only when the basemap is empty, so a fresh deploy populates itself and a
/// restart is a no-op. Uses a dedicated batch pool (no statement timeout).
async fn maybe_provision_on_boot(
    serving_db: turbo_tiles_db::DbPool,
    area: String,
    mvt_cache: turbo_tiles_api::mvt_tile_cache::MvtTileCache,
    raster_cache: turbo_tiles_api::mvt_tile_cache::MvtTileCache,
    basemap_ready: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    // Cheap emptiness probe on the serving pool (short timeout is fine here).
    let already: i64 = sqlx::query_scalar("SELECT count(*) FROM terrain.water_polygon")
        .fetch_one(&serving_db)
        .await
        .unwrap_or(0);
    if already > 0 {
        basemap_ready.store(true, std::sync::atomic::Ordering::Relaxed);
        tracing::info!(
            rows = already,
            "boot-provision: basemap already populated, skipping"
        );
        return;
    }
    tracing::warn!(area = %area, "boot-provision: empty basemap — provisioning N50 from Geonorge");
    let mut cfg = match DbConfig::from_env() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(error = %e, "boot-provision: DATABASE_URL missing");
            return;
        }
    };
    cfg.statement_timeout_ms = 0; // batch job, not a serving query
    cfg.max_connections = 2;
    let pool = match cfg.connect().await {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(error = %e, "boot-provision: batch pool connect failed");
            return;
        }
    };
    let opts = turbo_tiles_ingest::JobOptions {
        area: Some(area),
        force: false,
        ..Default::default()
    };
    match turbo_tiles_ingest::run_job_with_options(
        pool,
        turbo_tiles_ingest::JobName::ProvisionN50,
        opts,
    )
    .await
    {
        Ok(o) => {
            // Tiles requested while the DB was empty got cached empty; drop them
            // so the now-populated data is served.
            mvt_cache.bump_version();
            raster_cache.bump_version();
            // Data has landed — flip the basemap endpoint from 503 to serving.
            basemap_ready.store(true, std::sync::atomic::Ordering::Relaxed);
            tracing::info!(rows = o.rows_upserted, "boot-provision: complete");
        }
        Err(e) => tracing::error!(error = %e, "boot-provision: failed"),
    }
}

/// Periodic refresh loop: every `interval`, re-provision the area currently
/// recorded in `paths.provision_state`. Cheap when the source is unchanged
/// (the freshness skip returns after download+hash); does a full refresh
/// when Kartverket republished. Uses a dedicated batch pool (no statement
/// timeout). Does nothing until something has been provisioned at least once.
async fn refresh_loop(
    serving_db: turbo_tiles_db::DbPool,
    interval: std::time::Duration,
    mvt_cache: turbo_tiles_api::mvt_tile_cache::MvtTileCache,
    raster_cache: turbo_tiles_api::mvt_tile_cache::MvtTileCache,
) {
    tracing::info!(
        secs = interval.as_secs(),
        "provision-refresh: scheduler armed"
    );
    let mut tick = tokio::time::interval(interval);
    tick.tick().await; // consume the immediate first tick; wait a full interval
    loop {
        tick.tick().await;
        let area = match turbo_tiles_ingest::provisioned_area(&serving_db).await {
            Ok(Some(a)) => a,
            Ok(None) => {
                tracing::debug!("provision-refresh: nothing provisioned yet, skipping tick");
                continue;
            }
            Err(e) => {
                tracing::warn!(error = %e, "provision-refresh: state read failed");
                continue;
            }
        };
        let mut cfg = match DbConfig::from_env() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(error = %e, "provision-refresh: DATABASE_URL missing");
                continue;
            }
        };
        cfg.statement_timeout_ms = 0;
        cfg.max_connections = 2;
        let pool = match cfg.connect().await {
            Ok(p) => p,
            Err(e) => {
                tracing::error!(error = %e, "provision-refresh: pool connect failed");
                continue;
            }
        };
        tracing::info!(area = %area, "provision-refresh: checking for a newer N50 dump");
        let opts = turbo_tiles_ingest::JobOptions {
            area: Some(area),
            force: false,
            ..Default::default()
        };
        match turbo_tiles_ingest::run_job_with_options(
            pool,
            turbo_tiles_ingest::JobName::ProvisionN50,
            opts,
        )
        .await
        {
            // Only invalidate when the refresh actually changed data (a freshness
            // skip upserts nothing) — so the cache isn't needlessly cleared each
            // tick when Kartverket hasn't republished.
            Ok(o) if o.rows_upserted > 0 => {
                mvt_cache.bump_version();
                raster_cache.bump_version();
                tracing::info!(
                    rows = o.rows_upserted,
                    "provision-refresh: updated, tile caches invalidated"
                );
            }
            Ok(_) => {}
            Err(e) => tracing::error!(error = %e, "provision-refresh: failed"),
        }
    }
}

async fn ingest(
    job: &str,
    bbox: Option<&str>,
    file: Option<std::path::PathBuf>,
    area: Option<String>,
    source: String,
    force: bool,
) -> Result<()> {
    let mut db_cfg = DbConfig::from_env().context("DATABASE_URL must be set")?;
    // Ingest is a batch operation (restore + upserts + topology rebuilds run
    // for minutes), not a serving query. Disable the per-statement timeout so
    // a cold-cache contour/vegnett upsert isn't killed by the 10 s serving
    // default. An operator watching the job log can still cancel.
    db_cfg.statement_timeout_ms = 0;
    let db = db_cfg.connect().await.context("connecting to database")?;
    let job: turbo_tiles_ingest::JobName = job.parse().map_err(|e: String| anyhow::anyhow!(e))?;
    let opts = turbo_tiles_ingest::JobOptions {
        bbox: bbox.map(parse_bbox).transpose()?,
        file,
        source: Some(source),
        area,
        run_id: None,
        force,
    };
    let outcome = turbo_tiles_ingest::run_job_with_options(db, job, opts).await?;
    println!(
        "{{\"job\":\"{}\",\"rows_in\":{},\"rows_upserted\":{}}}",
        job.as_str(),
        outcome.rows_in,
        outcome.rows_upserted
    );
    Ok(())
}

fn parse_bbox(s: &str) -> Result<turbo_tiles_ingest::Bbox> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 4 {
        anyhow::bail!("bbox must be `W,S,E,N` (got `{s}`)");
    }
    let west: f64 = parts[0].parse().context("bbox west")?;
    let south: f64 = parts[1].parse().context("bbox south")?;
    let east: f64 = parts[2].parse().context("bbox east")?;
    let north: f64 = parts[3].parse().context("bbox north")?;
    if west > east || south > north {
        anyhow::bail!("bbox must have west<=east and south<=north");
    }
    Ok(turbo_tiles_ingest::Bbox {
        west,
        south,
        east,
        north,
    })
}

async fn verify_artifacts(
    dir: std::path::PathBuf,
    baseline: Option<std::path::PathBuf>,
    baseline_pct: f64,
) -> Result<()> {
    // Probe each artifact: open it, then run one cheap sanity query
    // (sample / snap / nearest) that touches the actual data path.
    let mut report = serde_json::Map::new();
    let dem_path = dir.join("norway.dem");
    if dem_path.exists() {
        let r = match turbo_tiles_elev::Dem::open(&dem_path) {
            Ok(d) => {
                let cov = d.coverage();
                // Sample the bbox centre — exercises mmap + zstd decode.
                let cx = (cov.min_x + cov.max_x) / 2.0;
                let cy = (cov.min_y + cov.max_y) / 2.0;
                let sample = d.sample(turbo_tiles_elev::PointXY { x: cx, y: cy });
                serde_json::json!({
                    "ok": true,
                    "cells_x": cov.cells_x,
                    "cells_y": cov.cells_y,
                    "tiles_present": cov.tiles_present,
                    "centre_sample": format!("{:?}", sample),
                })
            }
            Err(e) => serde_json::json!({ "ok": false, "error": e.to_string() }),
        };
        report.insert("dem".into(), r);
    }
    let mask_path = dir.join("norway.mask");
    if mask_path.exists() {
        let r = match turbo_tiles_mask::Mask::open(&mask_path) {
            Ok(m) => {
                let cov = m.coverage();
                serde_json::json!({
                    "ok": true,
                    "cells_x": cov.meta.cells_x,
                    "cells_water": cov.cells_water,
                    "cells_glacier": cov.cells_glacier,
                })
            }
            Err(e) => serde_json::json!({ "ok": false, "error": e.to_string() }),
        };
        report.insert("mask".into(), r);
    }
    let graph_path = dir.join("norway.graph");
    if graph_path.exists() {
        let r = match turbo_tiles_graph::Graph::open(&graph_path) {
            Ok(g) => {
                let s = g.stats();
                let probe_snap = g.snap(
                    ((s.min_x + s.max_x) / 2.0) as f64,
                    ((s.min_y + s.max_y) / 2.0) as f64,
                    100_000.0,
                );
                serde_json::json!({
                    "ok": true,
                    "nodes": s.meta.node_count,
                    "edges": s.meta.edge_count,
                    "centre_snap_id": probe_snap.ok(),
                })
            }
            Err(e) => serde_json::json!({ "ok": false, "error": e.to_string() }),
        };
        report.insert("graph".into(), r);
    }
    let anchors_path = dir.join("norway.anchors");
    if anchors_path.exists() {
        let r = match turbo_tiles_search::Index::open(&anchors_path) {
            Ok(idx) => {
                let s = idx.stats();
                serde_json::json!({
                    "ok": true,
                    "anchors": s.meta.count,
                    "names_size": s.meta.names_size,
                    "by_kind": s.by_kind,
                })
            }
            Err(e) => serde_json::json!({ "ok": false, "error": e.to_string() }),
        };
        report.insert("anchors".into(), r);
    }
    let probed_any = !report.is_empty();
    // Sidecar health reports written by the builders. The verify
    // step surfaces them next to the per-artifact open-probe so
    // the operator gets cell/node/edge counts AND structural
    // warnings ("49195 sti components, largest 7.2%") in one
    // command.
    let mut health_block = serde_json::Map::new();
    // Mask sidecars are named per artifact (norway.mask.health.json,
    // norway.wetland.mask.health.json, …). Discover them by suffix
    // so all polygon-rasterised masks are surfaced together.
    let mut sidecars: Vec<(String, std::path::PathBuf)> = Vec::new();
    for (kind_name, fname) in [
        ("graph", "norway.graph.health.json"),
        ("vectors", "norway.vectors.health.json"),
        ("dem", "norway.dem.health.json"),
    ] {
        let p = dir.join(fname);
        if p.exists() {
            sidecars.push((kind_name.to_string(), p));
        }
    }
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for ent in rd.flatten() {
            let name = ent.file_name().to_string_lossy().into_owned();
            if name.ends_with(".mask.health.json") {
                // norway.mask.health.json → "mask",
                // norway.wetland.mask.health.json → "mask.wetland".
                let stem = name.trim_end_matches(".mask.health.json");
                let key = if stem == "norway" {
                    "mask".to_string()
                } else {
                    let variant = stem.trim_start_matches("norway.");
                    format!("mask.{variant}")
                };
                sidecars.push((key, ent.path()));
            }
        }
    }
    for (kind_name, p) in &sidecars {
        if let Ok(text) = std::fs::read_to_string(p) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                // Promote warnings/errors to a top-level list
                // so the operator's eye lands on them first.
                let warns = v
                    .pointer("/report/warnings")
                    .cloned()
                    .unwrap_or(serde_json::json!([]));
                let errs = v
                    .pointer("/report/errors")
                    .cloned()
                    .unwrap_or(serde_json::json!([]));
                health_block.insert(
                    kind_name.to_string(),
                    serde_json::json!({
                        "warnings": warns,
                        "errors": errs,
                        "written_at_unix_sec": v.get("written_at_unix_sec"),
                        "stats_keys": v
                            .pointer("/report/stats")
                            .and_then(|s| s.as_object())
                            .map(|o| o.keys().count())
                            .unwrap_or(0),
                    }),
                );
            }
        }
    }
    if !health_block.is_empty() {
        report.insert("health".into(), serde_json::Value::Object(health_block));
    }
    // Baseline drift: load matching sidecars from --baseline=PATH
    // and surface drifted stats + newly-appeared warnings. This is
    // the bridge between the per-build audit and CI — the build's
    // baseline JSON is checked into a known location, and drift
    // beyond `--baseline-pct` fails the verify step's eyeball
    // pass-through.
    if let Some(base_dir) = &baseline {
        let mut drift_block = serde_json::Map::new();
        for (kind_name, current_path) in &sidecars {
            let fname = match current_path.file_name() {
                Some(n) => n.to_owned(),
                None => continue,
            };
            let base_path = base_dir.join(&fname);
            if !base_path.exists() {
                continue;
            }
            let (cur_text, base_text) = match (
                std::fs::read_to_string(current_path),
                std::fs::read_to_string(&base_path),
            ) {
                (Ok(a), Ok(b)) => (a, b),
                _ => continue,
            };
            let cur_v: serde_json::Value = match serde_json::from_str(&cur_text) {
                Ok(v) => v,
                _ => continue,
            };
            let base_v: serde_json::Value = match serde_json::from_str(&base_text) {
                Ok(v) => v,
                _ => continue,
            };
            let cur_report: turbo_tiles_build::HealthReport =
                match serde_json::from_value(cur_v.pointer("/report").cloned().unwrap_or_default())
                {
                    Ok(r) => r,
                    _ => continue,
                };
            let base_report: turbo_tiles_build::HealthReport = match serde_json::from_value(
                base_v.pointer("/report").cloned().unwrap_or_default(),
            ) {
                Ok(r) => r,
                _ => continue,
            };
            let diff = cur_report.compare_to(&base_report, baseline_pct);
            if !diff.drifted.is_empty() || !diff.new_warnings.is_empty() {
                drift_block.insert(
                    kind_name.clone(),
                    serde_json::to_value(&diff).unwrap_or_default(),
                );
            }
        }
        if !drift_block.is_empty() {
            report.insert("drift".into(), serde_json::Value::Object(drift_block));
        } else {
            report.insert(
                "drift".into(),
                serde_json::json!({
                    "_note": format!("no drift > {baseline_pct}% vs {}", base_dir.display())
                }),
            );
        }
    }
    if !probed_any && !report.contains_key("health") && !report.contains_key("drift") {
        anyhow::bail!(
            "no artifacts or sidecar health reports found in {}",
            dir.display()
        );
    }
    println!("{}", serde_json::Value::Object(report));
    Ok(())
}

async fn build_artifacts(kind: &str, out: std::path::PathBuf) -> Result<()> {
    let mut db_cfg = DbConfig::from_env().context("DATABASE_URL must be set")?;
    // Artifact builds stream tens of thousands of rows through a single
    // transaction — the request-path default (10 s) is wildly wrong
    // for this workload. Bump to 30 minutes unless the operator has
    // *explicitly* overridden via DB_STATEMENT_TIMEOUT_MS in the env.
    // (Previously a full DEM build aborted at the 2 000-row mark with
    // `canceling statement due to statement timeout`.)
    if std::env::var("DB_STATEMENT_TIMEOUT_MS").is_err() {
        db_cfg.statement_timeout_ms = 30 * 60 * 1000;
        tracing::info!(
            statement_timeout_ms = db_cfg.statement_timeout_ms,
            "build-artifacts: raised statement_timeout for long-running streaming queries"
        );
    }
    let db = db_cfg.connect().await.context("connecting to database")?;
    let builder = turbo_tiles_build::Builder::new(db, out.clone());
    match kind {
        "dem" => {
            let report = builder.dem().await.context("building DEM artifact")?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        "graph" => {
            let report = builder.graph().await.context("building graph artifact")?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        "search" => {
            let report = builder.search().await.context("building search artifact")?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        "mask" => {
            let report = builder.mask().await.context("building mask artifact")?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        "wetland" | "forest" | "open" => {
            let report = builder
                .landcover(kind)
                .await
                .with_context(|| format!("building {kind} landcover artifact"))?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        "cultivated" | "developed" | "building" | "stream_barrier" | "bridge_zone" => {
            let report = builder
                .n50_landcover(kind)
                .await
                .with_context(|| format!("building {kind} N50 mask"))?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        "vectors" => {
            // Default config path; override with TURBO_VECTOR_CONFIG.
            let cfg_path = std::env::var("TURBO_VECTOR_CONFIG").unwrap_or_else(|_| {
                // Same dir layout as the rest of the build tools.
                "tools/vector-layers.toml".to_string()
            });
            let cfg_text = std::fs::read_to_string(&cfg_path)
                .with_context(|| format!("reading vector config {cfg_path}"))?;
            let cfg: turbo_tiles_build::VectorConfig = toml::from_str(&cfg_text)
                .with_context(|| format!("parsing vector config {cfg_path}"))?;
            let report = builder
                .vectors(&cfg)
                .await
                .context("building vectors artifact")?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        "all" => {
            let dem = builder.dem().await.context("building DEM artifact")?;
            let graph = builder.graph().await.context("building graph artifact")?;
            let search = builder.search().await.context("building search artifact")?;
            let mask = builder.mask().await.context("building mask artifact")?;
            println!(
                "{}",
                serde_json::json!({
                    "dem": dem,
                    "graph": graph,
                    "search": search,
                    "mask": mask,
                })
            );
        }
        other => anyhow::bail!("unknown build-artifacts kind: {other}"),
    }
    Ok(())
}

async fn migrate() -> Result<()> {
    let db_cfg = DbConfig::from_env().context("DATABASE_URL must be set")?;
    let db = db_cfg.connect().await.context("connecting to database")?;
    migrations::apply(&db).await.context("running migrations")?;
    tracing::info!("migrations applied");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("install ctrl-c handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutting down");
}
