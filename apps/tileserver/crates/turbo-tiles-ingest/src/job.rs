use std::str::FromStr;

use turbo_tiles_db::DbPool;

use crate::fkb_wfs::Bbox;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobName {
    FkbSti,
    Turbase,
    Dnt,
    Dtm10Attach,
}

impl FromStr for JobName {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "fkb-sti" => Ok(JobName::FkbSti),
            "turbase" => Ok(JobName::Turbase),
            "dnt" => Ok(JobName::Dnt),
            "dtm10-attach" => Ok(JobName::Dtm10Attach),
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
}

#[derive(Debug, Default, Clone)]
pub struct JobOptions {
    /// Optional ingest bbox for `fkb-sti`. None falls back to the
    /// job's own default (the Oslo demo bbox).
    pub bbox: Option<Bbox>,
}

pub async fn run_job(pool: &DbPool, job: JobName) -> Result<JobOutcome, JobError> {
    run_job_with_options(pool, job, JobOptions::default()).await
}

pub async fn run_job_with_options(
    pool: &DbPool,
    job: JobName,
    opts: JobOptions,
) -> Result<JobOutcome, JobError> {
    let run_id = uuid::Uuid::new_v4();
    tracing::info!(job = job.as_str(), %run_id, "starting ingest job");

    let job_row_id = open_job_row(pool, job, run_id).await?;

    let result = match job {
        JobName::FkbSti => match opts.bbox {
            Some(b) => crate::fkb_wfs::run_with_bbox(pool, b).await,
            None => crate::fkb_wfs::run(pool, run_id).await,
        },
        JobName::Dtm10Attach => crate::dtm10::run(pool).await,
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
    let row: (i64,) = sqlx::query_as(
        r#"
        INSERT INTO paths.ingest_job (run_id, name, status, started_at)
        VALUES ($1, $2, 'running', now())
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
        SET status = $2,
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
