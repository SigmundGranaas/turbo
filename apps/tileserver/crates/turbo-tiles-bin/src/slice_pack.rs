//! `tileserver slice-pack` — the CLI face of [`turbo_geodata_pack`].
//!
//! The slicing itself moved to that crate so the *server* can serve what
//! this command builds; what is left here is argument shape and the
//! report a person reads. A library that printed would be one the API
//! could not call without polluting its logs.

use std::path::Path;

pub use turbo_geodata_pack::Region;

pub fn run(
    src: &Path,
    dst: &Path,
    region: &Region,
    halo_m: f64,
) -> Result<(), Box<dyn std::error::Error>> {
    let r = turbo_geodata_pack::build(src, dst, region, halo_m)?;

    println!(
        "bbox (halo {halo_m:.0} m): [{:.0}, {:.0}] .. [{:.0}, {:.0}]  ({:.1} x {:.1} km)",
        r.bbox.min_x,
        r.bbox.min_y,
        r.bbox.max_x,
        r.bbox.max_y,
        (r.bbox.max_x - r.bbox.min_x) / 1000.0,
        (r.bbox.max_y - r.bbox.min_y) / 1000.0,
    );
    println!("verify: {} points agree with the source", r.verified_points);
    let e = r.manifest.pack.extent;
    println!(
        "manifest: v{}  extent [{:.4}, {:.4}] .. [{:.4}, {:.4}]  halo {:.0} m",
        r.manifest.pack.format_version, e[0], e[1], e[2], e[3], r.manifest.pack.halo_m
    );
    println!();
    for f in &r.manifest.pack.files {
        println!(
            "  {:<18} {:>8.1} MB  {}",
            f.name,
            f.bytes as f64 / 1e6,
            &f.sha256[..16]
        );
    }
    println!();
    for s in &r.shrink {
        println!(
            "  {:<14} {:>8.1} MB -> {:>6.1} MB  {:>7.2} s   {}",
            s.name,
            s.before as f64 / 1e6,
            s.after as f64 / 1e6,
            s.elapsed.as_secs_f64(),
            s.detail
        );
    }
    // Verify and manifest are phases too, and on a large source the
    // first is the expensive one — it re-opens the SOURCE dem, which
    // bulk-loads an r-tree over every tile in it. Printing only the
    // slice phases would leave the biggest number off the report.
    println!(
        "  {:<14} {:>8}    {:>6}   {:>7.2} s   {} points",
        "verify", "", "", r.verify_elapsed.as_secs_f64(), r.verified_points
    );
    println!(
        "  {:<14} {:>8}    {:>6}   {:>7.2} s   sha256 over the output",
        "manifest",
        "",
        "",
        r.manifest_elapsed.as_secs_f64()
    );
    let (b, a) = (r.before_bytes(), r.after_bytes());
    println!(
        "  {:<14} {:>8.1} MB -> {:>6.1} MB  {:>7.2} s   ({:.1}% of original)",
        "TOTAL",
        b as f64 / 1e6,
        a as f64 / 1e6,
        r.total_elapsed.as_secs_f64(),
        100.0 * a as f64 / b as f64
    );
    Ok(())
}
