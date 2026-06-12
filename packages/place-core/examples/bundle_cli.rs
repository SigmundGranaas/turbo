//! Standalone harness over the embedded engine — the product demo / smoke test
//! for the offline path (no server, no client app).
//!
//! ```text
//! cargo run --features embedded --example bundle_cli -- <bundle.sqlite> reverse <lat> <lng>
//! cargo run --features embedded --example bundle_cli -- <bundle.sqlite> search  <query> [limit]
//! cargo run --features embedded --example bundle_cli -- demo
//! ```
//!
//! `demo` builds a tiny self-contained bundle and runs the five terrain-type
//! queries from the sampling run, so the embedded engine can be exercised end
//! to end with zero external dependencies.

use place_core::Bundle;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.as_slice() {
        [cmd] if cmd == "demo" => demo(),
        [path, cmd, lat, lng] if cmd == "reverse" => reverse(path, lat, lng),
        [path, cmd, q] if cmd == "search" => search(path, q, 5),
        [path, cmd, q, limit] if cmd == "search" => search(path, q, limit.parse().unwrap_or(5)),
        _ => {
            eprintln!(
                "usage:\n  bundle_cli <bundle.sqlite> reverse <lat> <lng>\n  \
                 bundle_cli <bundle.sqlite> search <query> [limit]\n  bundle_cli demo"
            );
            1
        }
    };
    std::process::exit(code);
}

fn open(path: &str) -> Bundle {
    match Bundle::open(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("cannot open bundle {path}: {e:?}");
            std::process::exit(1);
        }
    }
}

fn reverse(path: &str, lat: &str, lng: &str) -> i32 {
    let (lat, lng) = (lat.parse().unwrap(), lng.parse().unwrap());
    let d = open(path).reverse(lat, lng).expect("reverse");
    println!("{}", render(d.as_ref()));
    0
}

fn search(path: &str, query: &str, limit: usize) -> i32 {
    let hits = open(path).search(query, limit).expect("search");
    if hits.is_empty() {
        println!("(no results)");
    }
    for h in hits {
        println!(
            "{} [{}]{}",
            h.title,
            h.icon,
            h.description.map(|d| format!(" ({d})")).unwrap_or_default()
        );
    }
    0
}

fn render(d: Option<&place_core::LocationDescription>) -> String {
    use place_core::Qualifier::*;
    let Some(d) = d else {
        return "(no result)".into();
    };
    let label = match d.qualifier {
        Some(On) => format!("On {}", d.title),
        Some(InArea) => format!("In {}", d.title),
        Some(AtPlace) => format!("At {}", d.title),
        Some(CloseTo) => format!("Close to {}", d.title),
        Some(Near) => format!("Near {}", d.title),
        None => d.title.clone(),
    };
    let mut parts = Vec::new();
    if let Some(s) = &d.secondary {
        parts.push(s.clone());
    }
    if let Some(e) = d.elevation_m {
        parts.push(format!("{} m", e.round() as i64));
    }
    let area: Vec<_> = [d.kommune.clone(), d.fylke.clone()]
        .into_iter()
        .flatten()
        .collect();
    if !area.is_empty() {
        parts.push(area.join(", "));
    }
    if parts.is_empty() {
        label
    } else {
        format!("{label} · {}", parts.join(" · "))
    }
}

/// Build a small bundle covering five terrain types and run a query at each —
/// the offline counterpart of the multi-region sampling verification.
fn demo() -> i32 {
    let path = std::env::temp_dir()
        .join("place-core-demo-bundle.sqlite")
        .to_string_lossy()
        .into_owned();
    let _ = std::fs::remove_file(&path);
    build_demo_bundle(&path);

    let bundle = open(&path);
    println!("== embedded bundle demo (offline, no server) ==");
    for (label, lat, lng) in [
        ("Galdhøpiggen summit", 61.6363, 8.3120),
        ("Tromsø domkirke", 69.6488, 18.9551),
        ("wilderness (park)", 61.5050, 8.4100),
    ] {
        let d = bundle.reverse(lat, lng).expect("reverse");
        println!("reverse @ {label:<20} -> {}", render(d.as_ref()));
    }
    for q in ["galdh", "troms"] {
        let hits = bundle.search(q, 3).expect("search");
        let rendered: Vec<_> = hits
            .iter()
            .map(|h| format!("{} [{}]", h.title, h.icon))
            .collect();
        println!("search  \"{q}\" -> {}", rendered.join("; "));
    }
    0
}

fn build_demo_bundle(path: &str) {
    use rusqlite::Connection;
    let conn = Connection::open(path).unwrap();
    conn.execute_batch(
        "CREATE TABLE ruleset(json TEXT);
         CREATE TABLE places(id INTEGER PRIMARY KEY, name TEXT, name_fold TEXT, kind TEXT,
             lat REAL, lng REAL, status TEXT, elevation_m REAL, kommune TEXT, fylke TEXT);
         CREATE VIRTUAL TABLE places_rtree USING rtree(id, minLat, maxLat, minLng, maxLng);
         CREATE TABLE areas(id INTEGER PRIMARY KEY, area_type TEXT, name TEXT, kind TEXT, rings_json TEXT);
         CREATE VIRTUAL TABLE areas_rtree USING rtree(id, minLat, maxLat, minLng, maxLng);",
    )
    .unwrap();
    conn.execute("INSERT INTO ruleset(json) VALUES (?1)", [RULESET])
        .unwrap();

    let places = [
        (
            1,
            "Galdhøpiggen",
            "Fjell",
            61.63644,
            8.31248,
            2469.0,
            "Lom",
            "Innlandet",
        ),
        (
            2,
            "Tromsø domkirke",
            "Kirke",
            69.64876,
            18.95659,
            8.0,
            "Tromsø",
            "Troms",
        ),
    ];
    for (id, name, kind, lat, lng, elev, kommune, fylke) in places {
        conn.execute(
            "INSERT INTO places(id,name,name_fold,kind,lat,lng,status,elevation_m,kommune,fylke)
             VALUES (?1,?2,?3,?4,?5,?6,'aktiv',?7,?8,?9)",
            rusqlite::params![
                id,
                name,
                name.to_lowercase(),
                kind,
                lat,
                lng,
                elev,
                kommune,
                fylke
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO places_rtree(id,minLat,maxLat,minLng,maxLng) VALUES (?1,?2,?2,?3,?3)",
            rusqlite::params![id, lat, lng],
        )
        .unwrap();
    }

    let rings = "[[[8.35,61.47],[8.47,61.47],[8.47,61.53],[8.35,61.53],[8.35,61.47]]]";
    conn.execute(
        "INSERT INTO areas(id,area_type,name,kind,rings_json) VALUES (1,'protected_area','Jotunheimen','Nasjonalpark',?1)",
        [rings],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO areas_rtree(id,minLat,maxLat,minLng,maxLng) VALUES (1,61.47,61.53,8.35,8.47)",
        [],
    )
    .unwrap();
}

const RULESET: &str = include_str!("../ruleset.v1.json");
