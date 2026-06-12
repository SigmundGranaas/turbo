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

pub mod dnt_cabins;
pub mod dtm10;
pub mod dtm_bulk;
pub mod dtm_raster;
pub mod edge_attrs;
pub mod fkb_wfs;
pub mod geonorge;
pub mod job;
pub mod n50;
pub mod n50_anchors;
pub mod pgdump_load;
pub mod provision;
pub mod recommend_seed;
pub mod skeleton_build;
pub mod stage;
pub mod turbase;

pub use dtm_raster::{incoming_dir, list_incoming, resolve_under_incoming};

pub use fkb_wfs::Bbox;
pub use job::{run_job, run_job_with_options, JobError, JobName, JobOptions, JobOutcome};
pub use provision::provisioned_area;
