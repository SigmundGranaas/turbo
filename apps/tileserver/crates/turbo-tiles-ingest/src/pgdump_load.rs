//! Generic PostgreSQL-dump restore.
//!
//! Kartverket distributes N50 Kartdata and Turrutebasen as `pg_dump`
//! plain-text SQL files (inside a zip). Each dump creates a schema
//! named `<dataset>_<random_hex>` — the hex differs per release so we
//! can't hardcode it. This module restores the dump then renames the
//! discovered schema to a canonical staging name (`n50_staging`,
//! `turbase_staging`) so per-target upsert SQL can reference fixed
//! table paths.
//!
//! Why not ogr2ogr: the dumps are *already* PostgreSQL — no format
//! translation needed. `psql -f` is faster than ogr2ogr, has no
//! external dependency beyond psql itself, and preserves Kartverket's
//! native column types (text[] arrays, jsonb fields, geometry typing
//! with proper SRID).
//!
//! Idempotency: if the canonical staging schema already exists, the
//! `force` flag controls whether to nuke it first. Default behaviour
//! preserves prior restores so the operator can re-run upserts cheaply
//! without re-paying the (slow) restore.

use std::path::PathBuf;
use std::process::Stdio;

use tokio::process::Command;
use turbo_tiles_db::DbPool;

use crate::job::{JobError, JobOutcome};

/// Per-restore configuration. All `'static` because configs live in
/// per-dataset modules.
#[derive(Debug, Clone, Copy)]
pub struct PgDumpConfig {
    /// LIKE-pattern that matches the dump's source schema name (e.g.
    /// `n50kartdata_%`).
    pub source_schema_pattern: &'static str,
    /// Final schema name after rename (e.g. `n50_staging`).
    pub canonical_schema: &'static str,
    /// Sentinel table the upsert jobs check for ("did the restore
    /// finish?"). Useful for pre-flight in downstream jobs.
    pub sentinel_table: &'static str,
    /// Human label for logs and admin job rows.
    pub label: &'static str,
}

/// Result of a successful restore.
#[derive(Debug, Clone)]
pub struct PgDumpRestoreResult {
    pub canonical_schema: String,
    pub source_schema: String,
    pub rows_restored: i64,
}

/// Restore a pg_dump SQL file into Postgres, then rename the
/// discovered schema to the configured canonical name.
pub async fn restore(
    pool: &DbPool,
    sql_path: PathBuf,
    config: PgDumpConfig,
    force: bool,
) -> Result<PgDumpRestoreResult, JobError> {
    if !sql_path.exists() {
        return Err(JobError::Fetch(format!(
            "dump file not found: {}",
            sql_path.display()
        )));
    }

    // 1. If a canonical staging schema already exists and `force` is
    // false, skip the restore — the operator can re-run upserts
    // cheaply without paying the 25 GB restore cost again.
    let already: Option<(String,)> = sqlx::query_as(
        "SELECT schema_name::text FROM information_schema.schemata WHERE schema_name = $1",
    )
    .bind(config.canonical_schema)
    .fetch_optional(pool)
    .await?;
    if already.is_some() && !force {
        tracing::info!(
            schema = config.canonical_schema,
            "pgdump-load: canonical staging schema already exists; skipping restore (use force to re-restore)"
        );
        let n = canonical_row_count(pool, config).await.unwrap_or(0);
        return Ok(PgDumpRestoreResult {
            canonical_schema: config.canonical_schema.to_string(),
            source_schema: config.canonical_schema.to_string(),
            rows_restored: n,
        });
    }
    if already.is_some() && force {
        tracing::info!(
            schema = config.canonical_schema,
            "pgdump-load: force=true, dropping existing canonical staging"
        );
        sqlx::query(&format!(
            "DROP SCHEMA IF EXISTS {} CASCADE",
            quote_ident(config.canonical_schema)
        ))
        .execute(pool)
        .await?;
    }

    // 2. Drop any prior hash-named schema matching the pattern so the
    // discovery step after restore returns a unique result.
    let prior: Vec<(String,)> = sqlx::query_as(
        "SELECT schema_name::text FROM information_schema.schemata WHERE schema_name LIKE $1",
    )
    .bind(config.source_schema_pattern)
    .fetch_all(pool)
    .await?;
    for (s,) in prior {
        tracing::info!(schema = %s, "pgdump-load: dropping prior hash-named schema");
        sqlx::query(&format!("DROP SCHEMA IF EXISTS {} CASCADE", quote_ident(&s)))
            .execute(pool)
            .await?;
    }

    // 3. Restore via psql. `-v ON_ERROR_STOP=1` so a malformed dump
    // fails fast; `-q` to silence the per-statement chatter (Kartverket
    // dumps have hundreds of thousands of COPY statements).
    let database_url = std::env::var("DATABASE_URL")
        .map_err(|_| JobError::Fetch("DATABASE_URL unset".into()))?;
    let file_str = sql_path
        .to_str()
        .ok_or_else(|| JobError::Fetch("non-UTF8 path".into()))?;
    tracing::info!(file = %sql_path.display(), "pgdump-load: starting psql restore (may take a while)");
    let status = Command::new("psql")
        .arg(&database_url)
        .arg("-v")
        .arg("ON_ERROR_STOP=1")
        .arg("-q")
        .arg("-f")
        .arg(file_str)
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .status()
        .await
        .map_err(|e| JobError::Fetch(format!("spawn psql: {e}")))?;
    if !status.success() {
        return Err(JobError::Fetch(format!(
            "psql exited with {status} while restoring {}",
            sql_path.display()
        )));
    }

    // 4. Discover the newly-created schema and rename to canonical.
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT schema_name::text FROM information_schema.schemata \
         WHERE schema_name LIKE $1 \
         ORDER BY schema_name DESC LIMIT 1",
    )
    .bind(config.source_schema_pattern)
    .fetch_optional(pool)
    .await?;
    let source_schema = row
        .map(|(s,)| s)
        .ok_or_else(|| {
            JobError::Parse(format!(
                "no schema matching `{}` after restore",
                config.source_schema_pattern
            ))
        })?;

    sqlx::query(&format!(
        "ALTER SCHEMA {} RENAME TO {}",
        quote_ident(&source_schema),
        quote_ident(config.canonical_schema)
    ))
    .execute(pool)
    .await?;
    tracing::info!(
        from = %source_schema,
        to = config.canonical_schema,
        "pgdump-load: renamed schema"
    );

    let n = canonical_row_count(pool, config).await.unwrap_or(0);
    Ok(PgDumpRestoreResult {
        canonical_schema: config.canonical_schema.to_string(),
        source_schema,
        rows_restored: n,
    })
}

/// Convenience for jobs that just want the outcome shape.
pub fn outcome_from_result(r: PgDumpRestoreResult) -> JobOutcome {
    JobOutcome {
        rows_in: r.rows_restored,
        rows_upserted: r.rows_restored,
    }
}

/// Pre-flight check: returns Err if the canonical staging schema
/// doesn't exist OR doesn't have the sentinel table. Per-target upsert
/// jobs call this to surface a helpful error when the restore was
/// skipped or failed.
pub async fn require_staging(pool: &DbPool, config: PgDumpConfig) -> Result<(), JobError> {
    let exists: Option<(bool,)> = sqlx::query_as(
        "SELECT EXISTS(SELECT 1 FROM information_schema.tables \
         WHERE table_schema = $1 AND table_name = $2)",
    )
    .bind(config.canonical_schema)
    .bind(config.sentinel_table)
    .fetch_optional(pool)
    .await?;
    match exists {
        Some((true,)) => Ok(()),
        _ => Err(JobError::Fetch(format!(
            "{}.{} not found — run the corresponding -restore job first",
            config.canonical_schema, config.sentinel_table
        ))),
    }
}

async fn canonical_row_count(pool: &DbPool, config: PgDumpConfig) -> Result<i64, JobError> {
    let sql = format!(
        "SELECT COUNT(*)::bigint FROM information_schema.tables WHERE table_schema = '{}'",
        config.canonical_schema
    );
    let (n,): (i64,) = sqlx::query_as(&sql).fetch_one(pool).await?;
    Ok(n)
}

/// Quote a PostgreSQL identifier safely (double-quote escape). Used
/// only on identifiers that come from `information_schema.schemata`
/// queries — Kartverket schema names are hex-stamped and contain no
/// quotes, but defence in depth.
fn quote_ident(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

/// Unzip a Kartverket pg_dump-format download. Returns the path of
/// the contained `.sql` file. Idempotent (unzip -n).
pub async fn unzip_dump(zip_path: &std::path::Path) -> Result<PathBuf, JobError> {
    if zip_path.extension().and_then(|s| s.to_str()) == Some("sql") {
        return Ok(zip_path.to_path_buf());
    }
    if zip_path.extension().and_then(|s| s.to_str()) != Some("zip") {
        return Err(JobError::Fetch(format!(
            "expected .zip or .sql, got: {}",
            zip_path.display()
        )));
    }
    let stem = zip_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| JobError::Fetch("zip has no stem".into()))?;
    let extract_dir = zip_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join(stem);
    std::fs::create_dir_all(&extract_dir)
        .map_err(|e| JobError::Fetch(format!("create extract dir: {e}")))?;
    let status = Command::new("unzip")
        .arg("-n")
        .arg("-q")
        .arg(zip_path)
        .arg("-d")
        .arg(&extract_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .status()
        .await
        .map_err(|e| JobError::Fetch(format!("spawn unzip: {e}")))?;
    if !status.success() {
        return Err(JobError::Fetch(format!("unzip exited with {status}")));
    }
    find_sql(&extract_dir).ok_or_else(|| {
        JobError::Fetch(format!(
            "no .sql file found under {}",
            extract_dir.display()
        ))
    })
}

fn find_sql(dir: &std::path::Path) -> Option<PathBuf> {
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let p = entry.path();
        if p.extension().and_then(|s| s.to_str()) == Some("sql") {
            return Some(p);
        }
        if p.is_dir() {
            if let Some(found) = find_sql(&p) {
                return Some(found);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_ident_escapes_double_quotes() {
        // Defence in depth — if a schema name ever contained a
        // double quote, we mustn't enable SQL injection.
        assert_eq!(quote_ident("plain"), "\"plain\"");
        assert_eq!(quote_ident("with\"quote"), "\"with\"\"quote\"");
    }

    #[test]
    fn unzip_dump_passthrough_for_sql_file() {
        // If the operator passes a .sql directly (after unzipping
        // manually), no extraction — return the path as-is.
        let rt = tokio::runtime::Runtime::new().unwrap();
        let p = std::path::PathBuf::from("/tmp/foo.sql");
        assert_eq!(rt.block_on(unzip_dump(&p)).unwrap(), p);
    }

    #[test]
    fn unzip_dump_rejects_unknown_extensions() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let p = std::path::PathBuf::from("/tmp/foo.tar");
        assert!(rt.block_on(unzip_dump(&p)).is_err());
    }

    #[test]
    fn find_sql_locates_nested_dump() {
        let root = std::env::temp_dir().join("pgdump-find-test");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("nested")).unwrap();
        std::fs::write(root.join("nested/dump.sql"), b"").unwrap();
        std::fs::write(root.join("readme.txt"), b"").unwrap();
        let found = find_sql(&root).expect("nested .sql should be found");
        assert!(found.ends_with("dump.sql"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
