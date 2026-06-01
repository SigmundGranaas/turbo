//! Knob calibration sweeper.
//!
//! Drives `tools/route-scenarios.toml` against a running tileserver
//! across a range of values for a single knob, prints a per-value
//! pass/fail table. Replaces "raise X from 1.5 to 1.7 because Q3
//! still wrong" — the curator runs `calibrate off_trail_base 1.0
//! 2.5 0.1`, reads the table, picks the max-pass value.
//!
//! Usage:
//!
//! ```bash
//! ./target/release/calibrate off_trail_base 1.0 2.5 0.1
//! ./target/release/calibrate off_trail_base 1.0 2.5 0.1 --host=http://localhost:8090
//! ```
//!
//! Supported knobs (each maps to a `Prefs` field of the same name):
//!   - `off_trail_base` (f64)
//!   - `snap_radius_m` (f32)
//!   - `bridge_radius_m` (f32)
//!   - `mesh_cell_m` (f64)
//!
//! Adding a new knob: extend the match in `apply_knob`. The full
//! corpus runs once per value; on a typical Norway artifact set
//! each value takes ~5 s (10 scenarios × ~500 ms median).

use std::collections::BTreeMap;
use std::process::ExitCode;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{json, Value};

const CORPUS_TOML: &str = include_str!("../../../../tools/route-scenarios.toml");
const DEFAULT_HOST: &str = "http://127.0.0.1:8090";

#[derive(Debug, Deserialize)]
struct Corpus {
    #[serde(rename = "scenario")]
    scenarios: Vec<Scenario>,
}

#[derive(Debug, Deserialize)]
struct Scenario {
    name: String,
    from: [f64; 2],
    to: [f64; 2],
    profile: String,
    #[serde(default)]
    snap_radius_m: Option<f32>,
    #[serde(default)]
    assert: Asserts,
}

#[derive(Debug, Default, Deserialize)]
struct Asserts {
    #[serde(default)]
    strategy_in: Option<Vec<String>>,
    #[serde(default)]
    length_m_min: Option<f64>,
    #[serde(default)]
    length_m_max: Option<f64>,
    #[serde(default)]
    fkb_breakdown_sti_pct_min: Option<f32>,
    #[serde(default)]
    fkb_breakdown_vei_pct_max: Option<f32>,
    #[serde(default)]
    must_fail: bool,
    #[serde(default)]
    must_fail_message_includes: Option<String>,
}

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let host = args
        .iter()
        .position(|a| a.starts_with("--host="))
        .map(|i| args.remove(i).trim_start_matches("--host=").to_string())
        .unwrap_or_else(|| {
            std::env::var("TURBO_TEST_HOST").unwrap_or_else(|_| DEFAULT_HOST.to_string())
        });

    if args.len() < 4 {
        eprintln!(
            "usage: calibrate <knob> <min> <max> <step> [--host=URL]\n  \
             e.g. calibrate off_trail_base 1.0 2.5 0.1"
        );
        return ExitCode::from(2);
    }
    let knob = args[0].clone();
    let min: f64 = args[1].parse().expect("min must be f64");
    let max: f64 = args[2].parse().expect("max must be f64");
    let step: f64 = args[3].parse().expect("step must be f64");
    if step <= 0.0 || max < min {
        eprintln!("bad range: min={min} max={max} step={step}");
        return ExitCode::from(2);
    }

    // Probe the host.
    if ureq::get(&format!("{host}/healthz"))
        .timeout(Duration::from_secs(2))
        .call()
        .is_err()
    {
        eprintln!("ERROR: tileserver not reachable at {host} — start one and rerun");
        return ExitCode::from(1);
    }

    let corpus: Corpus = toml::from_str(CORPUS_TOML).expect("route-scenarios.toml");
    eprintln!(
        "Calibrating `{knob}` over [{min:.2}, {max:.2}] step {step:.2} on {} scenarios",
        corpus.scenarios.len()
    );

    let mut values: Vec<f64> = Vec::new();
    let mut v = min;
    while v <= max + 1e-9 {
        values.push((v * 10000.0).round() / 10000.0);
        v += step;
    }

    // Per-value results: scenario name -> pass/fail.
    let mut grid: BTreeMap<String, BTreeMap<String, bool>> = BTreeMap::new();
    for sc in &corpus.scenarios {
        grid.insert(sc.name.clone(), BTreeMap::new());
    }
    let mut totals: BTreeMap<String, (u32, u32)> = BTreeMap::new(); // value -> (pass, fail)

    for v in &values {
        let key = fmt_v(*v);
        let mut pass = 0u32;
        let mut fail = 0u32;
        eprint!("{key:>8}  ");
        for sc in &corpus.scenarios {
            let ok = run_scenario(&host, sc, &knob, *v);
            let glyph = if ok { '✓' } else { '·' };
            eprint!("{glyph}");
            grid.get_mut(&sc.name).unwrap().insert(key.clone(), ok);
            if ok {
                pass += 1;
            } else {
                fail += 1;
            }
        }
        eprintln!("  {pass}/{}", pass + fail);
        totals.insert(key, (pass, fail));
    }
    eprintln!();

    // Per-scenario × value grid: each row a scenario, each column
    // a knob value. Easy to scan to see "Q3 starts passing at 1.6".
    let max_name = corpus
        .scenarios
        .iter()
        .map(|s| s.name.len())
        .max()
        .unwrap_or(0);
    print!("{:width$}", "scenario", width = max_name + 2);
    for v in &values {
        print!(" {:>5}", fmt_v(*v));
    }
    println!();
    for sc in &corpus.scenarios {
        print!("{:width$}", sc.name, width = max_name + 2);
        for v in &values {
            let key = fmt_v(*v);
            let ok = *grid[&sc.name].get(&key).unwrap_or(&false);
            print!(" {:>5}", if ok { "✓" } else { "·" });
        }
        println!();
    }
    println!();

    // Summary: max-pass value(s).
    let max_pass = totals.values().map(|(p, _)| *p).max().unwrap_or(0);
    let best: Vec<String> = totals
        .iter()
        .filter(|(_, (p, _))| *p == max_pass)
        .map(|(k, _)| k.clone())
        .collect();
    println!(
        "max corpus pass: {}/{} at {}={}",
        max_pass,
        corpus.scenarios.len(),
        knob,
        best.join(",")
    );
    ExitCode::SUCCESS
}

fn fmt_v(v: f64) -> String {
    format!("{v:.2}")
}

fn apply_knob(prefs: &mut Value, knob: &str, value: f64) -> bool {
    let obj = prefs.as_object_mut().expect("prefs must be object");
    match knob {
        "off_trail_base" => {
            obj.insert("off_trail_base".to_string(), json!(value));
            true
        }
        "snap_radius_m" => {
            obj.insert("snap_radius_m".to_string(), json!(value as f32));
            true
        }
        "bridge_radius_m" => {
            obj.insert("bridge_radius_m".to_string(), json!(value as f32));
            true
        }
        "mesh_cell_m" => {
            obj.insert("mesh_cell_m".to_string(), json!(value));
            true
        }
        _ => false,
    }
}

fn run_scenario(host: &str, sc: &Scenario, knob: &str, value: f64) -> bool {
    let mut prefs = json!({
        "profile": sc.profile,
        "snap_radius_m": sc.snap_radius_m.unwrap_or(300.0),
    });
    if !apply_knob(&mut prefs, knob, value) {
        eprintln!("unknown knob `{knob}` — skipping");
        return false;
    }
    let body = json!({
        "from": sc.from,
        "to": sc.to,
        "prefs": prefs,
    });
    let r = ureq::post(&format!("{host}/v1/pathfind"))
        .set("content-type", "application/json")
        .timeout(Duration::from_secs(90))
        .send_string(&body.to_string());

    match r {
        Ok(resp) => {
            if sc.assert.must_fail {
                return false;
            }
            let body_text = resp.into_string().unwrap_or_default();
            let parsed: Value = match serde_json::from_str(&body_text) {
                Ok(v) => v,
                Err(_) => return false,
            };
            let path = &parsed["path"];
            if let Some(strategies) = sc.assert.strategy_in.as_ref() {
                let s = path["strategy"].as_str().unwrap_or("");
                if !strategies.iter().any(|x| x == s) {
                    return false;
                }
            }
            let length = path["length_m"].as_f64().unwrap_or(0.0);
            if let Some(m) = sc.assert.length_m_min {
                if length < m {
                    return false;
                }
            }
            if let Some(m) = sc.assert.length_m_max {
                if length > m {
                    return false;
                }
            }
            let total = length.max(1.0);
            if let Some(min) = sc.assert.fkb_breakdown_sti_pct_min {
                let sti = path["fkb_breakdown"]["sti"].as_f64().unwrap_or(0.0);
                if ((sti / total * 100.0) as f32) < min {
                    return false;
                }
            }
            if let Some(max) = sc.assert.fkb_breakdown_vei_pct_max {
                let vei = path["fkb_breakdown"]["vei"].as_f64().unwrap_or(0.0);
                if ((vei / total * 100.0) as f32) > max {
                    return false;
                }
            }
            true
        }
        Err(ureq::Error::Status(_, resp)) => {
            if !sc.assert.must_fail {
                return false;
            }
            if let Some(needle) = sc.assert.must_fail_message_includes.as_ref() {
                let msg = resp.into_string().unwrap_or_default().to_lowercase();
                return msg.contains(&needle.to_lowercase());
            }
            true
        }
        Err(_) => false,
    }
}
