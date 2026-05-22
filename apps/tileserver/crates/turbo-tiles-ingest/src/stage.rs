//! Shared staging helpers reused by every job. Each job stages into its
//! own `paths.staging_<job>` table, then a generic upsert step compares
//! against `paths.edge` keyed on `attr_hash`.

use sha2::{Digest, Sha256};

/// Stable hash of an edge's geometry + key attributes. Same hash =
/// same edge (idempotent re-ingest is a no-op). Geometry is normalised
/// to WKT with 4 decimal places (~10 cm in EPSG:25833) before hashing
/// so floating-point noise on re-reads doesn't churn the table.
pub fn attr_hash(
    geom_wkt: &str,
    fkb_type: &str,
    marking: Option<&str>,
    surface: Option<&str>,
) -> String {
    let mut h = Sha256::new();
    h.update(geom_wkt.as_bytes());
    h.update(b"|");
    h.update(fkb_type.as_bytes());
    h.update(b"|");
    h.update(marking.unwrap_or("").as_bytes());
    h.update(b"|");
    h.update(surface.unwrap_or("").as_bytes());
    let result = h.finalize();
    hex::encode(result)
}
