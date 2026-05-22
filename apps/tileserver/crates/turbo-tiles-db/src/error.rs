use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("missing required env var: {0}")]
    MissingEnv(&'static str),
    #[error("database connection failed: {0}")]
    Connect(String),
    #[error("migration failed: {0}")]
    Migrate(String),
    #[error("query failed: {0}")]
    Query(#[from] sqlx::Error),
}
