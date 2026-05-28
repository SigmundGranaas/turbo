use std::io::Write;

use turbo_tiles_artifacts::{write_header, ArtifactKind, Header};
use turbo_tiles_search::{
    write_meta, AnchorKind, AnchorRecord, AnchorsMeta, Index, SEARCH_FORMAT_VERSION,
};

fn write_artifact(
    path: &std::path::Path,
    anchors: &[(u64, AnchorKind, f32, f32, f32, Option<&str>)],
) {
    let mut names_blob: Vec<u8> = Vec::new();
    let mut records: Vec<AnchorRecord> = Vec::with_capacity(anchors.len());
    for &(id, kind, x, y, elev, name) in anchors {
        let (off, len) = match name {
            Some(n) => {
                let off = names_blob.len() as u32;
                names_blob.extend_from_slice(n.as_bytes());
                (off, n.len() as u32)
            }
            None => (0, 0),
        };
        records.push(AnchorRecord {
            id,
            kind: kind as u32,
            x,
            y,
            elev_m: elev,
            name_off: off,
            name_len: len,
        });
    }
    let mut f = std::fs::File::create(path).unwrap();
    write_header(
        &mut f,
        &Header {
            kind: ArtifactKind::Anchors,
            format_version: SEARCH_FORMAT_VERSION,
            build_timestamp_unix_sec: 1_700_000_000,
        },
    )
    .unwrap();
    write_meta(
        &mut f,
        &AnchorsMeta {
            count: records.len() as u32,
            names_size: names_blob.len() as u64,
        },
    )
    .unwrap();
    let rb: &[u8] = bytemuck::cast_slice(&records);
    f.write_all(rb).unwrap();
    f.write_all(&names_blob).unwrap();
    f.sync_all().unwrap();
}

#[test]
fn nearest_returns_closest_first() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    write_artifact(
        tmp.path(),
        &[
            (1, AnchorKind::Summit, 0.0, 0.0, 1000.0, Some("A")),
            (2, AnchorKind::Summit, 100.0, 0.0, 800.0, Some("B")),
            (3, AnchorKind::Cabin, 50.0, 50.0, 0.0, Some("C")),
        ],
    );
    let idx = Index::open(tmp.path()).unwrap();
    let hits = idx.nearest(10.0, 0.0, None, 3);
    assert_eq!(hits.len(), 3);
    assert_eq!(hits[0].id, 1);
    assert_eq!(hits[1].id, 3);
    assert_eq!(hits[2].id, 2);
}

#[test]
fn nearest_with_kind_filter() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    write_artifact(
        tmp.path(),
        &[
            (1, AnchorKind::Summit, 0.0, 0.0, 1000.0, Some("A")),
            (2, AnchorKind::Cabin, 5.0, 5.0, 0.0, Some("B")),
        ],
    );
    let idx = Index::open(tmp.path()).unwrap();
    let hits = idx.nearest(0.0, 0.0, Some(AnchorKind::Cabin), 5);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].id, 2);
}

#[test]
fn search_name_substring() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    write_artifact(
        tmp.path(),
        &[
            (1, AnchorKind::Summit, 0.0, 0.0, 0.0, Some("Galdhøpiggen")),
            (2, AnchorKind::Cabin, 1.0, 0.0, 0.0, Some("Galdebu")),
            (3, AnchorKind::NamedPlace, 2.0, 0.0, 0.0, Some("Trondheim")),
        ],
    );
    let idx = Index::open(tmp.path()).unwrap();
    let hits = idx.search_name("Gald", 10);
    assert_eq!(hits.len(), 2);
    let names: Vec<String> = hits.iter().filter_map(|h| h.name.clone()).collect();
    assert!(names.contains(&"Galdebu".to_string()));
    assert!(names.contains(&"Galdhøpiggen".to_string()));
}

#[test]
fn stats_counts_by_kind() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    write_artifact(
        tmp.path(),
        &[
            (1, AnchorKind::Summit, 0.0, 0.0, 0.0, None),
            (2, AnchorKind::Summit, 1.0, 0.0, 0.0, None),
            (3, AnchorKind::Cabin, 2.0, 0.0, 0.0, None),
        ],
    );
    let idx = Index::open(tmp.path()).unwrap();
    let s = idx.stats();
    assert_eq!(s.by_kind.get("summit").unwrap().as_u64(), Some(2));
    assert_eq!(s.by_kind.get("cabin").unwrap().as_u64(), Some(1));
}
