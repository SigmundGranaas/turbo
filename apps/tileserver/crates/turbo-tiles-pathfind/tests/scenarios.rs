//! Routing scenario corpus test.
//!
//! Loads `tools/route-scenarios.toml` at compile time, then POSTs
//! each scenario to a running tileserver and checks the response
//! against the scenario's `[scenario.assert]` block.
//!
//! Why HTTP instead of opening artifacts in-process: the production
//! artifacts are 8+ GB across DEM, vectors, mask, graph. Opening
//! them per test crate would dominate the test runtime and tie the
//! corpus to whatever machine the artifacts are on. Driving the
//! live tileserver matches what the SPA does and keeps the test
//! lightweight.
//!
//! The test is **skip-on-unreachable** so `cargo test` on a clean
//! checkout (no server, no artifacts) doesn't fail. To run the
//! corpus locally:
//!
//! ```bash
//! ./target/release/tileserver serve --bind=127.0.0.1:8090 ...
//! cargo test --test scenarios -- --nocapture
//! ```
//!
//! Override the host with `TURBO_TEST_HOST=http://...`. The test
//! also accepts `TURBO_TEST_SCENARIO=<substring>` to run a single
//! scenario by name match (useful when iterating on one case).

use std::time::Duration;

use serde::Deserialize;

const DEFAULT_HOST: &str = "http://127.0.0.1:8090";
const CORPUS_TOML: &str = include_str!("../../../tools/route-scenarios.toml");

#[derive(Debug, Deserialize)]
struct Corpus {
    #[serde(rename = "scenario")]
    scenarios: Vec<Scenario>,
}

#[derive(Debug, Deserialize)]
struct Scenario {
    name: String,
    #[serde(default)]
    note: Option<String>,
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
    refused_by_must_include: Option<Vec<String>>,
    #[serde(default)]
    refused_by_must_not_include: Option<Vec<String>>,
    #[serde(default)]
    must_fail: bool,
    #[serde(default)]
    must_fail_message_includes: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PathfindResp {
    path: PathPayload,
}

#[derive(Debug, Deserialize)]
struct PathPayload {
    strategy: String,
    length_m: f64,
    #[serde(default)]
    fkb_breakdown: std::collections::BTreeMap<String, f64>,
    #[serde(default)]
    refused_by: Vec<String>,
}

/// `Ok` when the assertions held; `Err(msg)` describes the first
/// violation. The corpus runner aggregates all failures into one
/// panic message so the curator sees them together.
fn run_scenario(host: &str, sc: &Scenario) -> Result<(), String> {
    // TURBO_TEST_COST_MODE pins the solver mode for an A/B run.
    // When unset, the field is omitted so the server picks its
    // default — currently `walk_seconds` post-Stage-2. Explicit
    // values (`walk_seconds` or `multiplicative`) are forwarded.
    let mut prefs = serde_json::json!({
        "profile": sc.profile,
        "snap_radius_m": sc.snap_radius_m.unwrap_or(300.0),
    });
    if let Ok(cm) = std::env::var("TURBO_TEST_COST_MODE") {
        prefs["cost_mode"] = serde_json::Value::String(cm);
    }
    let body = serde_json::json!({
        "from": sc.from,
        "to": sc.to,
        "prefs": prefs,
    });

    let r = ureq::post(&format!("{host}/v1/pathfind"))
        .set("content-type", "application/json")
        .timeout(Duration::from_secs(90))
        .send_string(&body.to_string());

    let must_fail = sc.assert.must_fail;
    let body_text = match r {
        Ok(resp) => {
            let status = resp.status();
            let body_text = resp
                .into_string()
                .map_err(|e| format!("body read failed: {e}"))?;
            if must_fail {
                return Err(format!(
                    "expected failure, got HTTP {} with body: {}",
                    status,
                    body_text.chars().take(160).collect::<String>()
                ));
            }
            body_text
        }
        Err(ureq::Error::Status(code, resp)) => {
            let msg = resp.into_string().unwrap_or_default();
            if must_fail {
                if let Some(needle) = sc.assert.must_fail_message_includes.as_ref() {
                    if !msg.to_lowercase().contains(&needle.to_lowercase()) {
                        return Err(format!(
                            "failure message missing `{needle}`: HTTP {code} {msg}"
                        ));
                    }
                }
                return Ok(());
            }
            return Err(format!("HTTP {code}: {msg}"));
        }
        Err(e) => return Err(format!("transport error: {e}")),
    };

    let resp: PathfindResp = serde_json::from_str(&body_text)
        .map_err(|e| format!("response not PathfindResp: {e}\n  body: {body_text:.200}"))?;

    if let Some(allowed) = sc.assert.strategy_in.as_ref() {
        if !allowed.contains(&resp.path.strategy) {
            return Err(format!(
                "strategy `{}` not in allowed {:?}",
                resp.path.strategy, allowed
            ));
        }
    }
    if let Some(min) = sc.assert.length_m_min {
        if resp.path.length_m < min {
            return Err(format!(
                "length_m {:.0} < min {:.0}",
                resp.path.length_m, min
            ));
        }
    }
    if let Some(max) = sc.assert.length_m_max {
        if resp.path.length_m > max {
            return Err(format!(
                "length_m {:.0} > max {:.0}",
                resp.path.length_m, max
            ));
        }
    }
    let total = resp.path.length_m.max(1.0);
    if let Some(min) = sc.assert.fkb_breakdown_sti_pct_min {
        let sti = resp.path.fkb_breakdown.get("sti").copied().unwrap_or(0.0);
        let pct = (sti / total) * 100.0;
        if (pct as f32) < min {
            return Err(format!(
                "fkb_breakdown sti pct {:.0}% < min {:.0}% (sti={:.0}m/total={:.0}m)",
                pct, min, sti, total
            ));
        }
    }
    if let Some(max) = sc.assert.fkb_breakdown_vei_pct_max {
        let vei = resp.path.fkb_breakdown.get("vei").copied().unwrap_or(0.0);
        let pct = (vei / total) * 100.0;
        if (pct as f32) > max {
            return Err(format!(
                "fkb_breakdown vei pct {:.0}% > max {:.0}%",
                pct, max
            ));
        }
    }
    if let Some(must) = sc.assert.refused_by_must_include.as_ref() {
        for needle in must {
            if !resp.path.refused_by.contains(needle) {
                return Err(format!(
                    "refused_by missing `{needle}`; got {:?}",
                    resp.path.refused_by
                ));
            }
        }
    }
    if let Some(forbidden) = sc.assert.refused_by_must_not_include.as_ref() {
        for needle in forbidden {
            if resp.path.refused_by.contains(needle) {
                return Err(format!(
                    "refused_by contains `{needle}`; got {:?}",
                    resp.path.refused_by
                ));
            }
        }
    }
    Ok(())
}

fn host() -> String {
    std::env::var("TURBO_TEST_HOST").unwrap_or_else(|_| DEFAULT_HOST.to_string())
}

fn server_up(host: &str) -> bool {
    ureq::get(&format!("{host}/healthz"))
        .timeout(Duration::from_secs(2))
        .call()
        .is_ok()
}

#[test]
fn route_scenarios() {
    let host = host();
    if !server_up(&host) {
        eprintln!(
            "SKIP route_scenarios — no tileserver at {host}; start one with \
             `./target/release/tileserver serve --bind=127.0.0.1:8090 \
             --artifacts-dir=/tmp/turbo-norway` and rerun"
        );
        return;
    }

    let corpus: Corpus =
        toml::from_str(CORPUS_TOML).expect("route-scenarios.toml failed to parse");

    let filter = std::env::var("TURBO_TEST_SCENARIO").ok();
    let scenarios: Vec<&Scenario> = corpus
        .scenarios
        .iter()
        .filter(|s| filter.as_ref().map(|f| s.name.contains(f)).unwrap_or(true))
        .collect();

    let mut failures: Vec<String> = Vec::new();
    let mut passes: Vec<String> = Vec::new();
    for sc in &scenarios {
        eprint!("  • {} … ", sc.name);
        match run_scenario(&host, sc) {
            Ok(()) => {
                eprintln!("ok");
                passes.push(sc.name.clone());
            }
            Err(e) => {
                eprintln!("FAIL: {e}");
                failures.push(format!("{}: {e}", sc.name));
            }
        }
    }
    eprintln!("\nscenarios: {} pass, {} fail", passes.len(), failures.len());

    if !failures.is_empty() {
        panic!(
            "{} scenario failure(s):\n  - {}",
            failures.len(),
            failures.join("\n  - ")
        );
    }
}
