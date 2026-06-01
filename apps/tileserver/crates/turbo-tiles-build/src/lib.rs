//! Build-time CLI library. Reads Postgres staging, writes artifacts
//! that primitive crates mmap at runtime.
//!
//! Per-kind builders land here. Stage 1: `dem`.

mod dem_builder;
mod graph_builder;
pub mod health;
mod mask_builder;
mod search_builder;
pub mod vector_builder;

pub use health::{
    audit_graph, audit_vector_layer, DriftedStat, HealthDiff, HealthIssue, HealthReport,
};

pub use dem_builder::DemBuildReport;
pub use graph_builder::GraphBuildReport;
pub use mask_builder::MaskBuildReport;
pub use search_builder::SearchBuildReport;
pub use vector_builder::{
    VectorAttrSpec, VectorBuildReport, VectorCollectionReport, VectorConfig, VectorLayerSpec,
};

use std::path::PathBuf;
use thiserror::Error;
use turbo_tiles_db::DbPool;

#[derive(Debug, Error)]
pub enum BuildError {
    #[error("not yet implemented")]
    NotYetImplemented,
    #[error("db: {0}")]
    Db(#[from] sqlx::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("artifact: {0}")]
    Artifact(#[from] turbo_tiles_artifacts::ArtifactError),
    #[error("build: {0}")]
    Logic(String),
}

pub struct Builder {
    pool: DbPool,
    out_dir: PathBuf,
}

impl Builder {
    pub fn new(pool: DbPool, out_dir: PathBuf) -> Self {
        Self { pool, out_dir }
    }

    /// Build `norway.dem` from the `paths.dem` PostGIS raster staging.
    /// See [`dem_builder`] for the on-disk format + algorithm.
    pub async fn dem(&self) -> Result<DemBuildReport, BuildError> {
        dem_builder::build(&self.pool, &self.out_dir).await
    }

    pub async fn mask(&self) -> Result<MaskBuildReport, BuildError> {
        mask_builder::build(&self.pool, &self.out_dir).await
    }

    /// Build a single-class landcover mask (wetland, forest, …)
    /// reading from `terrain.landcover_patch.class = '<class>'`.
    /// Output file: `norway.<class>.mask` under the artifact dir.
    pub async fn landcover(&self, class: &str) -> Result<MaskBuildReport, BuildError> {
        mask_builder::build_landcover(&self.pool, &self.out_dir, class).await
    }

    /// N50-derived landcover masks pulled directly from staging
    /// tables (not from the canonical landcover_patch). One mask
    /// can union multiple source tables — e.g. `developed` blends
    /// `tettbebyggelse` (built-up areas) and `bygning_omrade`
    /// (individual buildings).
    pub async fn n50_landcover(&self, name: &str) -> Result<MaskBuildReport, BuildError> {
        use mask_builder::PolygonSource;
        // Per-kind resolution. Water shoreline and stream barriers
        // need fine cells (10 m); landcover stays coarse (100 m).
        // 10-m water masks fix the "50 m halo around every lake"
        // problem — a small lake now occupies its actual outline,
        // not its outline expanded by ½ a 100 m cell on each side.
        let resolution_m: f32 = match name {
            "stream_barrier" | "bridge_zone" => 10.0,
            _ => 100.0,
        };
        let sources: &[PolygonSource] = match name {
            "cultivated" => &[PolygonSource {
                schema: "n50_staging",
                table: "dyrketmark",
                geom_column: "omrade",
                geom_expression: None,
                where_clause: None,
            }],
            // Stream barrier: a tight 2-m buffer around the stream
            // centerline (was 5 m). With 10 m mask cells the buffer
            // contributes < 1 cell on each side instead of fattening
            // every stream into a wider barrier. Streams remain
            // fordable (×4) — the cost discourages crossing but
            // never forbids it.
            "stream_barrier" => &[PolygonSource {
                schema: "n50_staging",
                table: "elvbekk",
                geom_column: "senterlinje",
                geom_expression: Some("ST_Buffer(senterlinje, 2.0)"),
                where_clause: None,
            }],
            // Bridge-permission grid: a 10-m buffer (was 30 m).
            // Tighter so bridges only cancel the stream barrier
            // exactly where they cross. The bridge mask is at 10 m
            // resolution to match the stream mask grid.
            "bridge_zone" => &[PolygonSource {
                schema: "n50_staging",
                table: "veglenke",
                geom_column: "senterlinje",
                geom_expression: Some("ST_Buffer(senterlinje, 10.0)"),
                where_clause: Some("medium='L'"),
            }],
            // Union the built-up footprint with the building outlines
            // so the mask is dense in urbanised areas. Excludes
            // individual rural buildings? — no, includes them all;
            // that's intentional, hikers don't want to route through
            // a farmyard.
            "developed" => &[
                PolygonSource {
                    schema: "n50_staging",
                    table: "tettbebyggelse",
                    geom_column: "omrade",
                    geom_expression: None,
                    where_clause: None,
                },
                PolygonSource {
                    schema: "n50_staging",
                    table: "bymessigbebyggelse",
                    geom_column: "omrade",
                    geom_expression: None,
                    where_clause: None,
                },
            ],
            "building" => &[PolygonSource {
                schema: "n50_staging",
                table: "bygning_omrade",
                geom_column: "omrade",
                geom_expression: None,
                where_clause: None,
            }],
            _ => {
                return Err(BuildError::Logic(format!(
                    "unknown n50_landcover kind: {name}"
                )))
            }
        };
        mask_builder::build_from_polygons(&self.pool, &self.out_dir, name, sources, resolution_m)
            .await
    }
    pub async fn graph(&self) -> Result<GraphBuildReport, BuildError> {
        graph_builder::build(&self.pool, &self.out_dir).await
    }
    pub async fn search(&self) -> Result<SearchBuildReport, BuildError> {
        search_builder::build(&self.pool, &self.out_dir).await
    }

    /// Build `norway.vectors` from a TOML config (one row per named
    /// feature collection). Adding a new feature class is a config
    /// change — no Rust code required.
    pub async fn vectors(&self, config: &VectorConfig) -> Result<VectorBuildReport, BuildError> {
        vector_builder::build_from_config(&self.pool, &self.out_dir, config).await
    }
}
