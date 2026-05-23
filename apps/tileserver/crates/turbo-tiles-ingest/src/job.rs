use std::path::PathBuf;
use std::str::FromStr;

use turbo_tiles_db::DbPool;

use crate::fkb_wfs::Bbox;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobName {
    FkbSti,
    Turbase,
    Dnt,
    Dtm10Attach,
    /// Bulk-load a DTM GeoTIFF (or any raster2pgsql-compatible file)
    /// into `paths.dem`. Operator points `--file` at a `.tif` on the
    /// configured incoming volume.
    DtmLoad,
}

impl FromStr for JobName {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "fkb-sti" => Ok(JobName::FkbSti),
            "turbase" => Ok(JobName::Turbase),
            "dnt" => Ok(JobName::Dnt),
            "dtm10-attach" => Ok(JobName::Dtm10Attach),
            "dtm-load" => Ok(JobName::DtmLoad),
            other => Err(format!("unknown job `{other}`")),
        }
    }
}

impl JobName {
    pub fn as_str(self) -> &'static str {
        match self {
            JobName::FkbSti => "fkb-sti",
            JobName::Turbase => "turbase",
            JobName::Dnt => "dnt",
            JobName::Dtm10Attach => "dtm10-attach",
            JobName::DtmLoad => "dtm-load",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum JobError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error("fetch failed: {0}")]
    Fetch(String),
    #[error("parse failed: {0}")]
    Parse(String),
    #[error("not yet implemented: {0}")]
    NotImplemented(&'static str),
    #[error("missing required option: {0}")]
    MissingOption(&'static str),
}

#[derive(Debug, Default, Clone)]
pub struct JobOptions {
    /// Optional ingest bbox for `fkb-sti`.
    pub bbox: Option<Bbox>,
    /// Filesystem path for bulk-file jobs (e.g. `dtm-load`).
    pub file: Option<PathBuf>,
    /// Optional source label stamped on loaded rows (e.g. "dtm10").
    pub source: Option<String>,
    /// Caller-supplied run id. When set, the job will reuse it in the
    /// `paths.ingest_job` log row instead of generating a fresh UUID.
    /// Used by the admin bulk-trigger endpoint so the response carries
    /// a run_id that matches the eventual job row, letting the SPA
    /// poll for the specific job it triggered.
    pub run_id: Option<uuid::Uuid>,
    /// For `dtm10-attach`: re-attach elevation to every edge, even
    /// those that already have a value. Used when a higher-resolution
    /// DEM lands and the curator wants to overwrite older guesses.
    pub force: bool,
}

pub async fn run_job(pool: &DbPool, job: JobName) -> Result<JobOutcome, JobError> {
    run_job_with_options(pool, job, JobOptions::default()).await
}

pub async fn run_job_with_options(
    pool: &DbPool,
    job: JobName,
    opts: JobOptions,
) -> Result<JobOutcome, JobError> {
    let run_id = opts.run_id.unwrap_or_else(uuid::Uuid::new_v4);
    tracing::info!(job = job.as_str(), %run_id, "starting ingest job");

    let job_row_id = open_job_row(pool, job, run_id).await?;

    let result = match job {
        JobName::FkbSti => match opts.bbox {
            Some(b) => crate::fkb_wfs::run_with_bbox(pool, b).await,
            None => crate::fkb_wfs::run(pool, run_id).await,
        },
        JobName::Dtm10Attach => {
            if opts.force {
                crate::dtm10::run_force(pool).await
            } else {
                crate::dtm10::run(pool).await
            }
        }
        JobName::DtmLoad => {
            let file = opts.file.ok_or(JobError::MissingOption("file"))?;
            let source = opts.source.as_deref().unwrap_or("dtm10");
            crate::dtm_raster::load_geotiff(pool, &file, source).await
        }
        other => Err(JobError::NotImplemented(other.as_str())),
    };

    let outcome = match result {
        Ok(o) => {
            close_job_row(pool, job_row_id, "succeeded", &o, None).await?;
            o
        }
        Err(e) => {
            let outcome = JobOutcome::default();
            close_job_row(pool, job_row_id, "failed", &outcome, Some(&e.to_string())).await?;
            return Err(e);
        }
    };

    tracing::info!(job = job.as_str(), %run_id, ?outcome, "ingest job finished");
    Ok(outcome)
}

#[derive(Debug, Default)]
pub struct JobOutcome {
    pub rows_in: i64,
    pub rows_upserted: i64,
}

async fn open_job_row(pool: &DbPool, job: JobName, run_id: uuid::Uuid) -> Result<i64, sqlx::Error> {
    // `status` is an enum (paths.job_status); cast the bound text.
    let row: (i64,) = sqlx::query_as(
        r#"
        INSERT INTO paths.ingest_job (run_id, name, status, started_at)
        VALUES ($1, $2, 'running'::paths.job_status, now())
        RETURNING id
        "#,
    )
    .bind(run_id)
    .bind(job.as_str())
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

async fn close_job_row(
    pool: &DbPool,
    job_row_id: i64,
    status: &str,
    outcome: &JobOutcome,
    error: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE paths.ingest_job
        SET status = $2::paths.job_status,
            finished_at = now(),
            rows_in = $3,
            rows_upserted = $4,
            error_text = $5
        WHERE id = $1
        "#,
    )
    .bind(job_row_id)
    .bind(status)
    .bind(outcome.rows_in)
    .bind(outcome.rows_upserted)
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}
