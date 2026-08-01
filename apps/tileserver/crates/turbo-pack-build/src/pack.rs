//! Write the pack manifest, plus the provenance a device-built pack
//! needs and a server-built one does not.
//!
//! `PackManifest` already answers "what area" and "can this build read
//! it". A pack cut on a phone raises a third question the server never
//! had to: *where did these bytes come from?* The server's artifacts
//! come from one ingest pipeline whose state an operator can inspect. A
//! device-built pack was assembled from whatever Kartverket was serving
//! at that moment, by whichever app version the user happens to have —
//! and when one routes oddly, the first useful question is which of
//! those two it was.
//!
//! Provenance is written as a sidecar (`provenance.toml`) rather than
//! into `pack.toml`, because `pack.toml`'s shape is a contract with
//! every client already shipped and `PackMeta` denies unknown fields on
//! neither side but gains nothing from carrying build trivia.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use turbo_tiles_artifacts::{PackFile, PackManifest, PackMeta, PACK_FORMAT_VERSION};

use crate::BuildError;

/// How a pack was produced.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Provenance {
    /// `wcs` / `n50-gml` / `fkb-wfs`, with the URL each was read from.
    pub sources: Vec<Source>,
    /// When the sources were read. The reason a rebuild of the same
    /// bbox can differ: Kartverket republishes.
    pub built_at: String,
    pub built_by: String,
    /// Metres per DEM sample that was *requested* — the WCS resamples
    /// from a 1 m source, so this is a choice, not a property of the
    /// data.
    pub dem_resolution_m: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Source {
    pub kind: String,
    pub url: String,
    /// Kommune numbers, for the sources scoped that way.
    #[serde(default)]
    pub areas: Vec<String>,
}

fn digest(path: &Path) -> Result<(u64, String), BuildError> {
    let bytes = std::fs::read(path)?;
    let mut h = Sha256::new();
    h.update(&bytes);
    Ok((bytes.len() as u64, format!("{:x}", h.finalize())))
}

/// Write `pack.toml` describing every artifact in `dir`.
///
/// The file list is read from the directory rather than passed in, so a
/// manifest cannot claim a file the pack does not have — the failure
/// that turns into a download stuck at 99% on someone's phone.
pub fn write_manifest(
    dir: &Path,
    extent_wgs84: [f64; 4],
    halo_m: f64,
    created_by: &str,
) -> Result<PathBuf, BuildError> {
    let mut files = Vec::new();
    let mut names: Vec<String> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.starts_with("norway."))
        .filter(|n| !n.ends_with(".tmp"))
        .collect();
    // Sorted, so two builds of the same region produce the same manifest
    // rather than one that differs by directory-iteration order.
    names.sort();

    for name in names {
        let (bytes, sha256) = digest(&dir.join(&name))?;
        files.push(PackFile {
            name,
            bytes,
            sha256,
        });
    }
    if files.is_empty() {
        return Err(BuildError::Logic(format!(
            "no norway.* artifacts in {} — refusing to write a manifest for an empty pack",
            dir.display()
        )));
    }

    let manifest = PackManifest {
        pack: PackMeta {
            format_version: PACK_FORMAT_VERSION,
            created_by: created_by.to_string(),
            frame: "utm33n".to_string(),
            extent: extent_wgs84,
            halo_m,
            files,
        },
    };
    let toml = toml::to_string_pretty(&manifest)
        .map_err(|e| BuildError::Logic(format!("serialise pack.toml: {e}")))?;
    let path = dir.join(PackManifest::FILENAME);
    std::fs::write(&path, toml)?;
    Ok(path)
}

pub fn write_provenance(dir: &Path, p: &Provenance) -> Result<PathBuf, BuildError> {
    let toml = toml::to_string_pretty(p)
        .map_err(|e| BuildError::Logic(format!("serialise provenance: {e}")))?;
    let path = dir.join("provenance.toml");
    std::fs::write(&path, toml)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_manifest_lists_every_artifact_with_its_digest() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("norway.dem"), b"dem bytes").unwrap();
        std::fs::write(dir.path().join("norway.mask"), b"mask").unwrap();
        // Not an artifact, must not be listed.
        std::fs::write(dir.path().join("notes.txt"), b"x").unwrap();

        let path = write_manifest(dir.path(), [15.0, 66.8, 15.5, 67.0], 1000.0, "test").unwrap();
        let parsed: PackManifest = toml::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();

        let names: Vec<&str> = parsed.pack.files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["norway.dem", "norway.mask"]);
        assert_eq!(parsed.pack.files[0].bytes, 9);
        assert_eq!(
            parsed.pack.files[0].sha256,
            format!("{:x}", Sha256::digest(b"dem bytes"))
        );
        assert_eq!(parsed.pack.frame, "utm33n");
        assert_eq!(parsed.pack.halo_m, 1000.0);
    }

    /// A half-written pack must not get a manifest that vouches for it.
    #[test]
    fn refuses_to_describe_an_empty_pack() {
        let dir = tempfile::tempdir().unwrap();
        assert!(write_manifest(dir.path(), [0.0; 4], 0.0, "test").is_err());
    }

    /// In-progress files are not artifacts. Listing one would put a
    /// digest in the manifest for bytes that are about to be renamed.
    #[test]
    fn ignores_tmp_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("norway.dem"), b"a").unwrap();
        std::fs::write(dir.path().join("norway.graph.tmp"), b"b").unwrap();
        let path = write_manifest(dir.path(), [0.0; 4], 0.0, "test").unwrap();
        let parsed: PackManifest = toml::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(parsed.pack.files.len(), 1);
    }

    /// Two builds of the same pack must produce the same manifest, or
    /// "did this change?" is unanswerable by comparison.
    #[test]
    fn the_file_order_is_stable() {
        let dir = tempfile::tempdir().unwrap();
        for n in ["norway.mask", "norway.dem", "norway.graph"] {
            std::fs::write(dir.path().join(n), n.as_bytes()).unwrap();
        }
        let a = std::fs::read_to_string(write_manifest(dir.path(), [0.0; 4], 0.0, "test").unwrap())
            .unwrap();
        let b = std::fs::read_to_string(write_manifest(dir.path(), [0.0; 4], 0.0, "test").unwrap())
            .unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn provenance_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let p = Provenance {
            sources: vec![Source {
                kind: "wcs".into(),
                url: crate::wcs::DEFAULT_ENDPOINT.into(),
                areas: vec![],
            }],
            built_at: "2026-08-01T00:00:00Z".into(),
            built_by: "test".into(),
            dem_resolution_m: 10.0,
        };
        let path = write_provenance(dir.path(), &p).unwrap();
        let back: Provenance = toml::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(back.sources[0].kind, "wcs");
        assert_eq!(back.dem_resolution_m, 10.0);
    }
}
