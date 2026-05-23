use sqlx::migrate::Migrator;

/// SQLx-embedded migrations. Files under `apps/tileserver/migrations/`
/// are baked into the binary at compile time and run by
/// `apply()` at service startup.
pub static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

pub async fn apply(pool: &super::DbPool) -> Result<(), super::DbError> {
    MIGRATOR
        .run(pool)
        .await
        .map_err(|e| super::DbError::Migrate(e.to_string()))
}
