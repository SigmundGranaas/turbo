//! Trace-format tests. No GPU needed, so these run in the default
//! `cargo test --workspace` lane and gate the serialisation contract.

use std::path::PathBuf;

use turbomap_golden::Trace;

fn traces_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("traces")
}

#[test]
fn bundled_traces_parse_and_roundtrip() {
    let mut count = 0;
    for entry in std::fs::read_dir(traces_dir()).expect("read traces dir") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let json = std::fs::read_to_string(&path).expect("read trace");
        let trace =
            Trace::from_json(&json).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()));
        assert!(
            !trace.layers.is_empty(),
            "{} declares no layers",
            path.display()
        );
        assert!(
            trace.width > 0 && trace.height > 0,
            "{} has a zero dimension",
            path.display()
        );
        // Round-trips through serde without losing the name/stack shape.
        let reparsed = Trace::from_json(&trace.to_json()).expect("reparse own json");
        assert_eq!(reparsed.name, trace.name);
        assert_eq!(reparsed.layers.len(), trace.layers.len());
        count += 1;
    }
    assert!(count >= 1, "no trace fixtures found in {:?}", traces_dir());
}
