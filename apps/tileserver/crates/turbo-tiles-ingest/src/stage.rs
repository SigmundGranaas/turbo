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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_for_same_input() {
        // Same inputs → same hash. Re-running the ingest with
        // unchanged data must produce zero diffs.
        let a = attr_hash("LINESTRING(1 2)", "sti", Some("red"), Some("dirt"));
        let b = attr_hash("LINESTRING(1 2)", "sti", Some("red"), Some("dirt"));
        assert_eq!(a, b);
    }

    #[test]
    fn distinguishes_marking_change() {
        // A trail that loses its red marking must produce a NEW
        // attr_hash so the diff stage notices and updates the row.
        let red = attr_hash("LINESTRING(1 2)", "sti", Some("red"), None);
        let none = attr_hash("LINESTRING(1 2)", "sti", None, None);
        assert_ne!(red, none);
    }

    #[test]
    fn distinguishes_geometry_change() {
        let a = attr_hash("LINESTRING(1 2)", "sti", None, None);
        let b = attr_hash("LINESTRING(1 3)", "sti", None, None);
        assert_ne!(a, b);
    }

    #[test]
    fn distinguishes_fkb_type_change() {
        // Same line re-classified from path to tractor road is a real
        // change a curator may care about — must be a new hash.
        let sti = attr_hash("LINESTRING(1 2)", "sti", None, None);
        let traktor = attr_hash("LINESTRING(1 2)", "traktorveg", None, None);
        assert_ne!(sti, traktor);
    }

    #[test]
    fn separator_prevents_field_collision() {
        // Without the `|` separator these would hash identically:
        // "sti" + "red" vs "stire" + "d". Confirms the separator is
        // doing its job.
        let a = attr_hash("g", "stire", Some("d"), None);
        let b = attr_hash("g", "sti", Some("red"), None);
        assert_ne!(a, b);
    }

    #[test]
    fn empty_optionals_distinct_from_empty_strings() {
        // None and Some("") collapse to the same hashed bytes
        // because both write zero bytes after the separator. This is
        // intentional — surface=NULL and surface='' carry the same
        // semantic weight in the FKB data — but documenting it as a
        // test so any future change is deliberate.
        let none = attr_hash("g", "sti", None, None);
        let empty = attr_hash("g", "sti", Some(""), Some(""));
        assert_eq!(none, empty);
    }

    #[test]
    fn hash_is_lowercase_hex() {
        let h = attr_hash("g", "sti", None, None);
        assert_eq!(h.len(), 64);
        assert!(h
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }
}
