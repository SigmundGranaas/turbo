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
        Command::Ingest { job } => ingest(&job).await,
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

async fn ingest(job: &str) -> Result<()> {
    let db_cfg = DbConfig::from_env().context("DATABASE_URL must be set")?;
    let db = db_cfg.connect().await.context("connecting to database")?;
    let job: turbo_tiles_ingest::JobName = job.parse().map_err(|e: String| anyhow::anyhow!(e))?;
    let outcome = turbo_tiles_ingest::run_job(&db, job).await?;
    println!(
        "{{\"job\":\"{}\",\"rows_in\":{},\"rows_upserted\":{}}}",
        job.as_str(),
        outcome.rows_in,
        outcome.rows_upserted
    );
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
