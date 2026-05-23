use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

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
        Command::Ingest { job, bbox } => ingest(&job, bbox.as_deref()).await,
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

    let app = axum::Router::new()
        .merge(api_router)
        .nest("/admin", admin_router);

    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .with_context(|| format!("binding {bind}"))?;
    tracing::info!(%bind, %public_base_url, "tileserver listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn ingest(job: &str, bbox: Option<&str>) -> Result<()> {
    let db_cfg = DbConfig::from_env().context("DATABASE_URL must be set")?;
    let db = db_cfg.connect().await.context("connecting to database")?;
    let job: turbo_tiles_ingest::JobName = job.parse().map_err(|e: String| anyhow::anyhow!(e))?;
    let opts = turbo_tiles_ingest::JobOptions {
        bbox: bbox.map(parse_bbox).transpose()?,
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
