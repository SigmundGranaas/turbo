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
    /// Bulk-load a single DTM GeoTIFF into `paths.dem` (low-level).
    DtmLoad,
    /// Bulk-extract a Kartverket DTM10 zip and load every .tif tile.
    DtmBulkLoad,
    /// Anchors ingest with baked dev fixture (replaces N50 when DB is empty).
    N50Anchors,
    /// Per-edge slope/aspect derivation from `paths.dem`.
    EdgeAttrs,
    /// Load the recommendation dev fixture.
    RecommendSeed,
    /// Synthesise off-trail "skeleton" edges via Delaunay over anchors.
    SkeletonBuild,
    /// Heavy: psql-restore the N50 dump into `n50_staging`.
    N50Restore,
    /// Cheap: re-run vann upsert against `n50_staging`.
    N50VannUpsert,
    /// Cheap: re-run glacier upsert against `n50_staging`.
    N50IsogBreUpsert,
    /// Cheap: re-run landcover upsert (skog/myr/apentomrade/dyrketmark).
    N50LandcoverUpsert,
    /// Cheap: re-run stedsnavn upsert against `n50_staging`.
    N50StedsnavnUpsert,
    /// Cheap: re-run vegnett upsert against `n50_staging`.
    N50VegnettUpsert,
    /// Heavy: psql-restore Turrutebasen dump into `turbase_staging`.
    TurbaseRestore,
    /// Cheap: re-run turbase upsert against `turbase_staging`.
    TurbaseUpsert,
    /// DNT cabins HTTP API → anchors.anchor.
    DntCabinsLoad,
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
            "dtm-bulk-load" => Ok(JobName::DtmBulkLoad),
            "n50-anchors" => Ok(JobName::N50Anchors),
            "edge-attrs" => Ok(JobName::EdgeAttrs),
            "recommend-seed" => Ok(JobName::RecommendSeed),
            "skeleton-build" => Ok(JobName::SkeletonBuild),
            "n50-restore" => Ok(JobName::N50Restore),
            "n50-vann-upsert" => Ok(JobName::N50VannUpsert),
            "n50-isogbre-upsert" => Ok(JobName::N50IsogBreUpsert),
            "n50-landcover-upsert" => Ok(JobName::N50LandcoverUpsert),
            "n50-stedsnavn-upsert" => Ok(JobName::N50StedsnavnUpsert),
            "n50-vegnett-upsert" => Ok(JobName::N50VegnettUpsert),
            "turbase-restore" => Ok(JobName::TurbaseRestore),
            "turbase-upsert" => Ok(JobName::TurbaseUpsert),
            "dnt-cabins-load" => Ok(JobName::DntCabinsLoad),
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
            JobName::DtmBulkLoad => "dtm-bulk-load",
            JobName::N50Anchors => "n50-anchors",
            JobName::EdgeAttrs => "edge-attrs",
            JobName::RecommendSeed => "recommend-seed",
            JobName::SkeletonBuild => "skeleton-build",
            JobName::N50Restore => "n50-restore",
            JobName::N50VannUpsert => "n50-vann-upsert",
            JobName::N50IsogBreUpsert => "n50-isogbre-upsert",
            JobName::N50LandcoverUpsert => "n50-landcover-upsert",
            JobName::N50StedsnavnUpsert => "n50-stedsnavn-upsert",
            JobName::N50VegnettUpsert => "n50-vegnett-upsert",
            JobName::TurbaseRestore => "turbase-restore",
            JobName::TurbaseUpsert => "turbase-upsert",
            JobName::DntCabinsLoad => "dnt-cabins-load",
        }
    }

    /// True iff the job accepts a `--file` option.
    pub fn takes_file(self) -> bool {
        matches!(
            self,
            JobName::DtmLoad
                | JobName::DtmBulkLoad
                | JobName::N50Anchors
                | JobName::N50Restore
                | JobName::TurbaseRestore
                | JobName::DntCabinsLoad
        )
    }

    pub fn all() -> &'static [JobName] {
        &[
            JobName::FkbSti,
            JobName::Turbase,
            JobName::Dnt,
            JobName::Dtm10Attach,
            JobName::DtmLoad,
            JobName::DtmBulkLoad,
            JobName::N50Anchors,
            JobName::EdgeAttrs,
            JobName::RecommendSeed,
            JobName::SkeletonBuild,
            JobName::N50Restore,
            JobName::N50VannUpsert,
            JobName::N50IsogBreUpsert,
            JobName::N50LandcoverUpsert,
            JobName::N50StedsnavnUpsert,
            JobName::N50VegnettUpsert,
            JobName::TurbaseRestore,
            JobName::TurbaseUpsert,
            JobName::DntCabinsLoad,
        ]
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
    pub bbox: Option<Bbox>,
    pub file: Option<PathBuf>,
    pub source: Option<String>,
    pub run_id: Option<uuid::Uuid>,
    /// Used for `dtm10-attach` (overwrite) and `n50-restore`/`turbase-restore`
    /// (drop+recreate the canonical staging schema).
    pub force: bool,
}

pub async fn run_job(pool: DbPool, job: JobName) -> Result<JobOutcome, JobError> {
    run_job_with_options_owned(pool, job, JobOptions::default()).await
}

pub async fn run_job_with_options(
    pool: DbPool,
    job: JobName,
    opts: JobOptions,
) -> Result<JobOutcome, JobError> {
    run_job_with_options_owned(pool, job, opts).await
}

async fn run_job_with_options_owned(
    pool: DbPool,
    job: JobName,
    opts: JobOptions,
) -> Result<JobOutcome, JobError> {
    let pool_ref = &pool;
    let run_id = opts.run_id.unwrap_or_else(uuid::Uuid::new_v4);
    tracing::info!(job = job.as_str(), %run_id, "starting ingest job");

    let job_row_id = open_job_row(pool_ref, job, run_id).await?;

    type FutResult = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<JobOutcome, JobError>> + Send>,
    >;
    let fut: FutResult = match job {
        JobName::FkbSti => {
            let p = pool.clone();
            let opts_bbox = opts.bbox;
            Box::pin(async move {
                match opts_bbox {
                    Some(b) => crate::fkb_wfs::run_with_bbox(&p, b).await,
                    None => crate::fkb_wfs::run(&p, run_id).await,
                }
            })
        }
        JobName::Dtm10Attach => {
            let p = pool.clone();
            let force = opts.force;
            Box::pin(async move {
                if force {
                    crate::dtm10::run_force(&p).await
                } else {
                    crate::dtm10::run(&p).await
                }
            })
        }
        JobName::DtmLoad => {
            let p = pool.clone();
            let file = opts.file.clone();
            let source = opts.source.clone().unwrap_or_else(|| "dtm10".to_string());
            Box::pin(async move {
                let file = file.ok_or(JobError::MissingOption("file"))?;
                crate::dtm_raster::load_geotiff(&p, &file, &source).await
            })
        }
        JobName::DtmBulkLoad => {
            let p = pool.clone();
            let file = opts.file.clone();
            let source = opts.source.clone().unwrap_or_else(|| "dtm10".to_string());
            Box::pin(async move {
                let file = file.ok_or(JobError::MissingOption("file"))?;
                crate::dtm_bulk::run(&p, file, source).await
            })
        }
        JobName::N50Anchors => {
            let p = pool.clone();
            let file = opts.file.clone();
            Box::pin(async move {
                match file {
                    Some(f) => crate::n50_anchors::run_from_file(&p, f).await,
                    None => crate::n50_anchors::run(&p, false).await,
                }
            })
        }
        JobName::EdgeAttrs => {
            let p = pool.clone();
            let force = opts.force;
            Box::pin(async move { crate::edge_attrs::run(&p, force).await })
        }
        JobName::RecommendSeed => {
            let p = pool.clone();
            Box::pin(async move { crate::recommend_seed::run(&p).await })
        }
        JobName::SkeletonBuild => {
            let p = pool.clone();
            Box::pin(async move { crate::skeleton_build::run(&p).await })
        }
        JobName::N50Restore => {
            let p = pool.clone();
            let file = opts.file.clone();
            let force = opts.force;
            Box::pin(async move {
                let file = file.ok_or(JobError::MissingOption("file"))?;
                crate::n50::restore(&p, file, force).await
            })
        }
        JobName::N50VannUpsert => {
            let p = pool.clone();
            Box::pin(async move { crate::n50::upsert_vann(&p).await })
        }
        JobName::N50IsogBreUpsert => {
            let p = pool.clone();
            Box::pin(async move { crate::n50::upsert_isogbre(&p).await })
        }
        JobName::N50LandcoverUpsert => {
            let p = pool.clone();
            Box::pin(async move { crate::n50::upsert_landcover(&p).await })
        }
        JobName::N50StedsnavnUpsert => {
            let p = pool.clone();
            Box::pin(async move { crate::n50::upsert_stedsnavn(&p).await })
        }
        JobName::N50VegnettUpsert => {
            let p = pool.clone();
            Box::pin(async move { crate::n50::upsert_vegnett(&p).await })
        }
        JobName::TurbaseRestore => {
            let p = pool.clone();
            let file = opts.file.clone();
            let force = opts.force;
            Box::pin(async move {
                let file = file.ok_or(JobError::MissingOption("file"))?;
                crate::turbase::restore(&p, file, force).await
            })
        }
        JobName::TurbaseUpsert => {
            let p = pool.clone();
            Box::pin(async move { crate::turbase::upsert(&p).await })
        }
        JobName::DntCabinsLoad => {
            let p = pool.clone();
            let file = opts.file.clone();
            Box::pin(async move { crate::dnt_cabins::run(&p, file, None).await })
        }
        other => Box::pin(async move { Err(JobError::NotImplemented(other.as_str())) }),
    };
    let result: Result<JobOutcome, JobError> = fut.await;

    let outcome = match result {
        Ok(o) => {
            close_job_row(pool_ref, job_row_id, "succeeded", &o, None).await?;
            o
        }
        Err(e) => {
            let outcome = JobOutcome::default();
            close_job_row(pool_ref, job_row_id, "failed", &outcome, Some(&e.to_string())).await?;
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
