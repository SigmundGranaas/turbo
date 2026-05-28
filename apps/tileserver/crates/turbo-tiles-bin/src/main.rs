use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

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
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
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
            source,
            force,
        } => ingest(&job, bbox.as_deref(), file, source, force).await,
        Command::BuildArtifacts { kind, out } => build_artifacts(&kind, out).await,
        Command::Migrate => migrate().await,
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
        tracing::warn!("--no-db: skipping DB connect and migrations; DB endpoints will 503 on demand");
        pool
    } else {
        let db_cfg = DbConfig::from_env().context("DATABASE_URL must be set")?;
        let pool = db_cfg.connect().await.context("connecting to database")?;
        if auto_migrate {
            tracing::info!("running migrations");
            migrations::apply(&pool).await.context("running migrations")?;
        }
        pool
    };

    let auth = AuthConfig::from_env().context("auth config")?;
    let auth_state = AuthState(Arc::new(auth.clone()));

    // Primitive handles are loaded once at boot from the artifact
    // directory. Missing artifacts leave the corresponding endpoint
    // in 503-degraded mode rather than failing the whole start-up.
    let mut api_state = ApiState::new(db.clone(), auth, public_base_url.clone());
    if let Some(dir) = artifacts_dir.as_ref() {
        let dem_path = dir.join("norway.dem");
        if dem_path.exists() {
            match turbo_tiles_elev::Dem::open(&dem_path) {
                Ok(d) => {
                    let cov = d.coverage();
                    tracing::info!(
                        path = %dem_path.display(),
                        cells_x = cov.cells_x,
                        cells_y = cov.cells_y,
                        tiles_present = cov.tiles_present,
                        tiles_absent = cov.tiles_absent,
                        file_size_bytes = cov.file_size_bytes,
                        "loaded DEM artifact"
                    );
                    api_state.dem = Some(std::sync::Arc::new(d));
                }
                Err(e) => tracing::error!(error = %e, path = %dem_path.display(), "failed to open DEM artifact; running in degraded mode"),
            }
        } else {
            tracing::warn!(path = %dem_path.display(), "DEM artifact not present; elev endpoints will return 503");
        }
        let mask_path = dir.join("norway.mask");
        if mask_path.exists() {
            match turbo_tiles_mask::Mask::open(&mask_path) {
                Ok(m) => {
                    let cov = m.coverage();
                    tracing::info!(
                        path = %mask_path.display(),
                        cells_x = cov.meta.cells_x,
                        cells_y = cov.meta.cells_y,
                        file_size_bytes = cov.file_size_bytes,
                        cells_water = cov.cells_water,
                        cells_glacier = cov.cells_glacier,
                        "loaded refusal mask artifact"
                    );
                    api_state.mask = Some(std::sync::Arc::new(m));
                }
                Err(e) => tracing::error!(error = %e, path = %mask_path.display(), "failed to open mask artifact"),
            }
        }
        let graph_path = dir.join("norway.graph");
        if graph_path.exists() {
            match turbo_tiles_graph::Graph::open(&graph_path) {
                Ok(mut g) => {
                    // Attach the polyline sibling artifact if it
                    // sits next to the graph. Missing/malformed is
                    // non-fatal — routes degrade to straight-segment
                    // geometry rather than failing.
                    let geom_path = dir.join("norway.graph_geom");
                    if geom_path.exists() {
                        match g.attach_geom(&geom_path) {
                            Ok(_) => tracing::info!(
                                path = %geom_path.display(),
                                "attached graph_geom artifact (high-fidelity polylines)"
                            ),
                            Err(e) => tracing::warn!(
                                error = %e,
                                path = %geom_path.display(),
                                "failed to attach graph_geom; routes will use endpoint segments"
                            ),
                        }
                    }
                    let s = g.stats();
                    tracing::info!(
                        path = %graph_path.display(),
                        nodes = s.meta.node_count,
                        edges = s.meta.edge_count,
                        file_size_bytes = s.file_size_bytes,
                        has_polylines = g.has_geom(),
                        "loaded routing graph artifact"
                    );
                    api_state.graph = Some(std::sync::Arc::new(g));
                }
                Err(e) => tracing::error!(error = %e, path = %graph_path.display(), "failed to open graph artifact"),
            }
        }
        let anchors_path = dir.join("norway.anchors");
        if anchors_path.exists() {
            match turbo_tiles_search::Index::open(&anchors_path) {
                Ok(s) => {
                    let st = s.stats();
                    tracing::info!(
                        path = %anchors_path.display(),
                        count = st.meta.count,
                        file_size_bytes = st.file_size_bytes,
                        "loaded anchor search artifact"
                    );
                    api_state.search = Some(std::sync::Arc::new(s));
                }
                Err(e) => tracing::error!(error = %e, path = %anchors_path.display(), "failed to open search artifact"),
            }
        }
    }

    // Assemble the Pathfinder with the default layer stack once we
    // know which primitives loaded. Then auto-discover any extra
    // landcover masks under the artifact directory and push them
    // as `LandcoverLayer`s. Known classes + their cost multipliers
    // are listed below — they're conservative defaults; a curator
    // can override at request time via `prefs.layer_weights`.
    // Resolve cost calibration knobs once at boot. Precedence:
    // explicit env var → cost-config.toml relative to CWD →
    // embedded defaults compiled into the binary. The same knobs
    // drive every layer's hardcoded values that previously had to
    // be touched in three or four files to recalibrate.
    let cost_config = turbo_tiles_pathfind::CostConfig::load_or_default(None)
        .unwrap_or_else(|e| {
            tracing::warn!(
                error = %e,
                "failed to load cost-config; falling back to embedded defaults"
            );
            turbo_tiles_pathfind::CostConfig::from_embedded()
                .expect("embedded cost-config defaults must parse")
        });
    tracing::info!(
        off_trail_base_foot = cost_config.off_trail_base.foot,
        proximity_bonus = cost_config.trail_proximity.bonus_at_zero,
        slope_refuse_cell = cost_config.slope_cell.refuse_above_deg,
        "loaded cost configuration"
    );
    let mut pf = turbo_tiles_pathfind::Pathfinder::with_defaults_and_config(
        api_state.dem.clone(),
        api_state.mask.clone(),
        api_state.graph.clone(),
        cost_config,
    );

    // ---- Vector cost layers ---------------------------------------------
    //
    // If `norway.vectors` is present, register the per-feature-class cost
    // layers from it. These supersede the equivalent rasterised mask
    // layers — water, wetland, streams, cultivated, building — because
    // they preserve the original polygon shape and integrate cost along
    // each candidate edge instead of vetoing whole 25m cells.
    let mut taken_layer_names: std::collections::HashSet<&'static str> =
        std::collections::HashSet::new();
    if let Some(dir) = artifacts_dir.as_ref() {
        let vec_path = dir.join("norway.vectors");
        if vec_path.exists() {
            match turbo_tiles_vector::VectorStore::open(&vec_path) {
                Ok(store) => {
                    tracing::info!(
                        path = %vec_path.display(),
                        collections = ?store.collection_names(),
                        "loaded vectors artifact"
                    );
                    // Polygon integral layers — cost per metre walked
                    // INSIDE the polygon. Returning a value in "extra
                    // effective metres" composes naturally with the
                    // Naismith metres-of-walk cost the rest of the
                    // system speaks in.
                    if let Some(coll) = store.try_collection("water") {
                        // 80×/m: a 5m crossing adds +400m; a 50m
                        // crossing adds +4000m. Detours under 800m
                        // and 8000m respectively pay for themselves.
                        let legacy = turbo_tiles_pathfind::PolygonIntegralLayer::new(
                            "water",
                            coll.clone(),
                            |len, _attrs, _p| len * 80.0,
                        );
                        let native = turbo_tiles_pathfind::PolygonIntegralContributor::new(
                            "water",
                            coll,
                            |len, _attrs, _p| len * 80.0,
                        );
                        pf.push_with_native(std::sync::Arc::new(legacy), std::sync::Arc::new(native));
                        taken_layer_names.insert("water");
                        // Tell the raster `mask_refusal` layer to
                        // stop vetoing water cells — the integral
                        // layer now handles them with proper
                        // edge-length cost. Without this swap the
                        // 25 m water bitmap still produces the
                        // halo-around-every-tarn pathology.
                        if let Some(m) = api_state.mask.clone() {
                            pf.defer_mask_water_to_vector(m);
                            tracing::info!(
                                "deferred raster mask water refusal to vector water layer"
                            );
                        }
                    }
                    if let Some(coll) = store.try_collection("wetland") {
                        let legacy = turbo_tiles_pathfind::PolygonIntegralLayer::new(
                            "wetland",
                            coll.clone(),
                            |len, _attrs, _p| len * 1.5,
                        );
                        let native = turbo_tiles_pathfind::PolygonIntegralContributor::new(
                            "wetland",
                            coll,
                            |len, _attrs, _p| len * 1.5,
                        );
                        pf.push_with_native(std::sync::Arc::new(legacy), std::sync::Arc::new(native));
                        taken_layer_names.insert("wetland");
                    }
                    if let Some(coll) = store.try_collection("cultivated") {
                        // Innmark — soft penalty so the solver routes
                        // around farmyards but doesn't refuse them.
                        let legacy = turbo_tiles_pathfind::PolygonIntegralLayer::new(
                            "cultivated",
                            coll.clone(),
                            |len, _attrs, _p| len * 3.0,
                        );
                        let native = turbo_tiles_pathfind::PolygonIntegralContributor::new(
                            "cultivated",
                            coll,
                            |len, _attrs, _p| len * 3.0,
                        );
                        pf.push_with_native(std::sync::Arc::new(legacy), std::sync::Arc::new(native));
                        taken_layer_names.insert("cultivated");
                    }
                    if let Some(coll) = store.try_collection("ocean") {
                        // Saltwater is a hard veto — you cannot
                        // wade across a fjord. Without this layer
                        // the mimicry harness's `bergen-2km-roads`
                        // case sent the mesh path right through
                        // Bergen harbor.
                        let legacy = turbo_tiles_pathfind::PolygonRefusalLayer::new(
                            "ocean",
                            coll.clone(),
                            "ocean",
                        );
                        let native = turbo_tiles_pathfind::PolygonRefusalContributor::new(
                            "ocean",
                            coll,
                            "ocean",
                        );
                        pf.push_with_native(std::sync::Arc::new(legacy), std::sync::Arc::new(native));
                        taken_layer_names.insert("ocean");
                    }
                    if let Some(coll) = store.try_collection("building") {
                        // Buildings are truly impassable — refusal
                        // layer, not integral. The collection acts as
                        // a high-fidelity replacement for the 100m
                        // building mask.
                        let legacy = turbo_tiles_pathfind::PolygonRefusalLayer::new(
                            "building",
                            coll.clone(),
                            "building",
                        );
                        let native = turbo_tiles_pathfind::PolygonRefusalContributor::new(
                            "building",
                            coll,
                            "building",
                        );
                        pf.push_with_native(std::sync::Arc::new(legacy), std::sync::Arc::new(native));
                        taken_layer_names.insert("building");
                    }
                    if let Some(coll) = store.try_collection("streams") {
                        // Stream crossings — width-aware cost. Width is
                        // metres; crossing cost = 10 + 5×width metres
                        // per crossing. A 1m brook = +15m; a 5m river
                        // = +35m; a 20m+ river is something else
                        // (typically broken into separate "river"
                        // polygons in the water layer anyway).
                        let legacy = turbo_tiles_pathfind::LineCrossingLayer::new(
                            "streams",
                            coll.clone(),
                            |n, attrs, _p| {
                                let w = attrs.f32("width_m").unwrap_or(2.0) as f64;
                                (n as f64) * (10.0 + 5.0 * w)
                            },
                        );
                        let native = turbo_tiles_pathfind::LineCrossingContributor::new(
                            "streams",
                            coll,
                            |n, attrs, _p| {
                                let w = attrs.f32("width_m").unwrap_or(2.0) as f64;
                                (n as f64) * (10.0 + 5.0 * w)
                            },
                        );
                        pf.push_with_native(std::sync::Arc::new(legacy), std::sync::Arc::new(native));
                        taken_layer_names.insert("streams");
                        // Mask-based stream_barrier + bridge_zone are
                        // strictly weaker than this — skip both.
                        taken_layer_names.insert("stream_barrier");
                        taken_layer_names.insert("bridge_zone");
                    }
                }
                Err(e) => tracing::error!(
                    error = %e,
                    path = %vec_path.display(),
                    "failed to open vectors artifact"
                ),
            }
        }
    }

    if let Some(dir) = artifacts_dir.as_ref() {
        // (file suffix, layer name, cost multiplier when class present)
        //
        // Multipliers calibrated for foot hiking. Ski/bike profiles
        // get the same multipliers in this stack — when those
        // profiles care about a different surface mix (e.g. ski
        // doesn't slow down in forest as much), a profile-aware
        // version of `LandcoverLayer` is the right place to add the
        // distinction.
        let landcover_specs: &[(&str, &'static str, f32)] = &[
            ("norway.wetland.mask", "wetland", 2.5),
            ("norway.forest.mask", "forest", 1.4),
            ("norway.open.mask", "open", 0.95),
            // Cultivated land (innmark): walkable per `allemannsretten`
            // only on existing paths. Heavy penalty when off-trail
            // so the solver routes around farmyards. Trail-proximity
            // bias counter-acts this when a real trail is nearby.
            ("norway.cultivated.mask", "cultivated", 4.0),
            // Built-up areas: passable but unattractive for hiking.
            ("norway.developed.mask", "developed", 2.5),
            // Individual buildings: refused — you can't walk THROUGH
            // a building. The mask cell is small (100 m) so the
            // refusal is roughly building-sized; routing detours
            // around it.
            ("norway.building.mask", "building", f32::INFINITY),
            // Stream barrier: fordable but expensive (4×). When a
            // bridge crosses the same cell, the `bridge_zone` layer
            // multiplies by 0.25 → product 1.0 (free crossing).
            ("norway.stream_barrier.mask", "stream_barrier", 4.0),
            ("norway.bridge_zone.mask", "bridge_zone", 0.25),
        ];
        for (filename, layer_name, multiplier) in landcover_specs {
            // Skip mask layers superseded by vector collections of
            // the same name — they're strictly worse (cell-grid all-
            // or-nothing) and would double-count cost if registered
            // alongside the vector layer.
            if taken_layer_names.contains(layer_name) {
                tracing::info!(
                    layer = layer_name,
                    "skipping mask landcover; vector layer present"
                );
                continue;
            }
            let path = dir.join(filename);
            if !path.exists() {
                continue;
            }
            match turbo_tiles_mask::Mask::open(&path) {
                Ok(m) => {
                    let cov = m.coverage();
                    tracing::info!(
                        path = %path.display(),
                        layer = layer_name,
                        present_cells = cov.cells_water,
                        multiplier,
                        "loaded landcover layer"
                    );
                    let arc = std::sync::Arc::new(m);
                    // Two consumers: the pathfinder layer stack
                    // (for cost composition) and the SPA inspect
                    // endpoints (for visualisation). Share via Arc.
                    api_state.landcover.insert(*layer_name, arc.clone());
                    let legacy = turbo_tiles_pathfind::LandcoverLayer {
                        mask: arc.clone(),
                        layer_name,
                        multiplier: *multiplier,
                    };
                    // Translate the legacy multiplier into a walk-
                    // seconds-per-metre delta against the flat-trail
                    // baseline. Infinity multipliers (building)
                    // become a hard veto on the native side via
                    // `LandcoverContributor::veto`.
                    let delta_s_per_m = if multiplier.is_infinite() {
                        f64::INFINITY
                    } else {
                        ((*multiplier as f64) - 1.0)
                            * turbo_tiles_pathfind::BASE_PACE_S_PER_M
                    };
                    let native = turbo_tiles_pathfind::LandcoverContributor::new(
                        arc,
                        layer_name,
                        delta_s_per_m,
                    );
                    pf.push_with_native(
                        std::sync::Arc::new(legacy),
                        std::sync::Arc::new(native),
                    );
                }
                Err(e) => tracing::error!(error = %e, path = %path.display(), "failed to open landcover artifact"),
            }
        }
    }
    tracing::info!(
        layers = ?pf.layer_names(),
        graph_loaded = api_state.graph.is_some(),
        "pathfinder assembled"
    );
    api_state.pathfinder = Some(std::sync::Arc::new(pf));
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

    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .with_context(|| format!("binding {bind}"))?;
    tracing::info!(%bind, %public_base_url, "tileserver listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn ingest(
    job: &str,
    bbox: Option<&str>,
    file: Option<std::path::PathBuf>,
    source: String,
    force: bool,
) -> Result<()> {
    let db_cfg = DbConfig::from_env().context("DATABASE_URL must be set")?;
    let db = db_cfg.connect().await.context("connecting to database")?;
    let job: turbo_tiles_ingest::JobName = job.parse().map_err(|e: String| anyhow::anyhow!(e))?;
    let opts = turbo_tiles_ingest::JobOptions {
        bbox: bbox.map(parse_bbox).transpose()?,
        file,
        source: Some(source),
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
            let cur_report: turbo_tiles_build::HealthReport = match serde_json::from_value(
                cur_v.pointer("/report").cloned().unwrap_or_default(),
            ) {
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
        anyhow::bail!("no artifacts or sidecar health reports found in {}", dir.display());
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
