use sqlx::postgres::{PgPoolOptions, PgSslMode};
use sqlx::ConnectOptions;
use std::time::Duration;

pub type DbPool = sqlx::PgPool;

#[derive(Debug, Clone)]
pub struct DbConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub statement_timeout_ms: u64,
}

impl DbConfig {
    pub fn from_env() -> Result<Self, super::DbError> {
        let url = std::env::var("DATABASE_URL")
            .map_err(|_| super::DbError::MissingEnv("DATABASE_URL"))?;
        let max_connections = std::env::var("DB_MAX_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(16);
        let min_connections = std::env::var("DB_MIN_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2);
        let statement_timeout_ms = std::env::var("DB_STATEMENT_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10_000);
        Ok(Self {
            url,
            max_connections,
            min_connections,
            statement_timeout_ms,
        })
    }

    /// Lazy pool: doesn't make a connection until a query asks for
    /// one. Used by `tileserver serve --no-db` where DB-touching
    /// endpoints are expected to 503 rather than fail-fast at boot.
    pub fn connect_lazy(&self) -> Result<DbPool, super::DbError> {
        let opts: sqlx::postgres::PgConnectOptions = self
            .url
            .parse()
            .map_err(|e: sqlx::Error| super::DbError::Connect(e.to_string()))?;
        let pool = PgPoolOptions::new()
            .max_connections(self.max_connections)
            .min_connections(0)
            .acquire_timeout(Duration::from_secs(2))
            .connect_lazy_with(opts);
        Ok(pool)
    }

    pub async fn connect(&self) -> Result<DbPool, super::DbError> {
        let mut opts: sqlx::postgres::PgConnectOptions = self
            .url
            .parse()
            .map_err(|e: sqlx::Error| super::DbError::Connect(e.to_string()))?;
        opts = opts.ssl_mode(PgSslMode::Prefer);
        opts = opts.log_statements(tracing::log::LevelFilter::Debug);

        let statement_timeout_ms = self.statement_timeout_ms;
        let pool = PgPoolOptions::new()
            .max_connections(self.max_connections)
            .min_connections(self.min_connections)
            .acquire_timeout(Duration::from_secs(10))
            .after_connect(move |conn, _meta| {
                Box::pin(async move {
                    sqlx::query(&format!("SET statement_timeout = {statement_timeout_ms}"))
                        .execute(conn)
                        .await?;
                    Ok(())
                })
            })
            .connect_with(opts)
            .await
            .map_err(|e| super::DbError::Connect(e.to_string()))?;
        Ok(pool)
    }
}
