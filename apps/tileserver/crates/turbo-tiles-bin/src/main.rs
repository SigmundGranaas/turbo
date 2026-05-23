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
        } => serve(bind, public_base_url, auto_migrate).await,
        Command::Ingest {
            job,
            bbox,
            file,
            source,
            force,
        } => ingest(&job, bbox.as_deref(), file, source, force).await,
        Command::Migrate => migrate().await,
    }
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,tower_http=debug,sqlx=warn"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .json()
        .with_target(true)
        .init();
}

async fn serve(bind: String, public_base_url: String, auto_migrate: bool) -> Result<()> {
    let db_cfg = DbConfig::from_env().context("DATABASE_URL must be set")?;
    let db = db_cfg.connect().await.context("connecting to database")?;

    if auto_migrate {
        tracing::info!("running migrations");
        migrations::apply(&db).await.context("running migrations")?;
    }

    let auth = AuthConfig::from_env().context("auth config")?;
    let auth_state = AuthState(Arc::new(auth.clone()));

    let api_state = ApiState::new(db.clone(), auth, public_base_url.clone());
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

    let app = axum::Router::new()
        .merge(api_router)
        .nest("/admin", admin_router)
        .nest_service("/admin/app", spa_service);

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
    let outcome = turbo_tiles_ingest::run_job_with_options(&db, job, opts).await?;
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
