//! Ingest pipeline. Pulled from Kartverket FKB, Nasjonal Turbase, DNT,
//! and DTM10. Each job is a stand-alone subcommand of the binary
//! (`tileserver ingest --job <name>`) and follows the same stages:
//!
//!   1. fetch   → idempotent download to local cache
//!   2. stage   → truncate-and-load into `paths.staging_<job>`
//!   3. diff    → compare against `paths.edge` by attr_hash
//!   4. upsert  → INSERT new, UPDATE changed, soft-delete missing
//!   5. topology→ rebuild via pgr_createTopology when geometry diffed
//!   6. refresh → materialised views if any
//!   7. log     → write `paths.ingest_job` row

pub mod fkb_wfs;
pub mod job;
pub mod stage;

pub use job::{run_job, JobError, JobName};
