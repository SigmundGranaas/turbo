//! Geometry hashes for the routes a host actually asks for.
//!
//! Run on two targets and compare. E1 established bit-identical results
//! between x86_64 and aarch64 under **glibc**; Android links **bionic**,
//! whose `libm` differs (E0 found an `f32::atan` divergence that did not
//! bite in 4 M samples). This is how that caveat gets closed without a
//! phone: build for `aarch64-linux-android` with a static bionic and run
//! it under qemu-user.
use turbo_route_ffi::{GeoPoint, RouteEngine, RouteOptions, TravelMode};

fn h(r: &turbo_route_ffi::Route) -> u64 {
    let mut x: u64 = 0xcbf2_9ce4_8422_2325;
    for p in &r.geometry {
        for b in ((p.lon * 1e7) as i64)
            .to_le_bytes()
            .iter()
            .chain(((p.lat * 1e7) as i64).to_le_bytes().iter())
        {
            x ^= *b as u64;
            x = x.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    x
}

/// Raw bits of the transcendentals the cost model actually calls.
///
/// Printed because the route hashes alone cannot tell "bionic agrees
/// with glibc" from "Rust never asked either of them". If these bits are
/// identical across two libcs on the same ISA, that is either genuine
/// agreement or a shared implementation — and either way the routes
/// above are comparing what they claim to. If they differ while the
/// routes still match, the solver is provably tolerant of the difference,
/// which is the stronger result.
///
/// `exp` is Tobler's; `atan`/`atan2` are slope and aspect; `sinh`/`tan`
/// are the Mercator projection the pack grid is named in. E0 measured an
/// `f32::atan` divergence between implementations, so it is included at
/// both widths.
fn libm_fingerprint() {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |bits: u64| {
        for b in bits.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    let mut i = 0;
    while i < 2000 {
        let x = -3.0 + (i as f64) * 0.003;
        mix((x.exp()).to_bits());
        mix((x.atan()).to_bits());
        mix((x.sinh()).to_bits());
        mix((x.tan()).to_bits());
        mix((x.atan2(1.7)).to_bits());
        mix(((x as f32).atan()).to_bits() as u64);
        mix(((x as f32).exp()).to_bits() as u64);
        i += 1;
    }
    println!("libm         {h:016x}  (exp/atan/sinh/tan/atan2, f64+f32, 2000 pts)");
}

fn main() {
    libm_fingerprint();
    let dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "tools/ci-pack".to_string());
    let e = RouteEngine::open(dir).expect("pack must open");
    let a = GeoPoint {
        lon: 15.04048,
        lat: 67.065016,
    };
    let b = GeoPoint {
        lon: 15.0555,
        lat: 67.0685,
    };

    let opts = || RouteOptions {
        mode: TravelMode::Foot,
        ..Default::default()
    };

    let base = e.plan(vec![a, b], opts()).unwrap();
    println!(
        "unified      {:016x}  {:.3} m  {} pts",
        h(&base),
        base.length_m,
        base.geometry.len()
    );

    let rt = e
        .plan(
            vec![a, b],
            RouteOptions {
                round_trip: true,
                ..opts()
            },
        )
        .unwrap();
    println!(
        "round_trip   {:016x}  {:.3} m  {} pts",
        h(&rt),
        rt.length_m,
        rt.geometry.len()
    );

    let av = e
        .plan(
            vec![a, b],
            RouteOptions {
                avoid: vec![base.geometry.clone()],
                avoid_radius_m: Some(50.0),
                ..opts()
            },
        )
        .unwrap();
    println!(
        "avoid        {:016x}  {:.3} m  {} pts",
        h(&av),
        av.length_m,
        av.geometry.len()
    );

    let ot = e
        .plan(
            vec![a, b],
            RouteOptions {
                force_off_trail: true,
                ..opts()
            },
        )
        .unwrap();
    println!(
        "off_trail    {:016x}  {:.3} m  {} pts",
        h(&ot),
        ot.length_m,
        ot.geometry.len()
    );
}
