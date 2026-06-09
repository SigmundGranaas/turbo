//! Runs the shared `golden.json` against `place-core`. This is the contract the
//! later FFI bindings (Dart / Kotlin / Swift / .NET) each smoke-test a subset of.

use place_core::{
    forward_search, reverse_geocode, LocationDescription, ReverseInput, Ruleset, SearchCandidate,
    SearchHit,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct GoldenCase {
    name: String,
    input: ReverseInput,
    expect: Option<LocationDescription>,
}

#[derive(Debug, Deserialize)]
struct SearchCase {
    name: String,
    query: String,
    candidates: Vec<SearchCandidate>,
    expect: Vec<SearchHit>,
}

#[test]
fn golden_reverse_geocode_cases() {
    let ruleset = Ruleset::load_default();
    let raw = include_str!("../golden.json");
    let cases: Vec<GoldenCase> =
        serde_json::from_str(raw).expect("golden.json parses into reverse-geocode cases");

    assert!(!cases.is_empty(), "golden.json must contain cases");

    let mut failures = Vec::new();
    for case in &cases {
        let got = reverse_geocode(&ruleset, &case.input);
        if got != case.expect {
            failures.push(format!(
                "case {:?}\n  expected: {:?}\n  got:      {:?}",
                case.name, case.expect, got
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "{} golden case(s) failed:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

#[test]
fn golden_forward_search_cases() {
    let ruleset = Ruleset::load_default();
    let raw = include_str!("../golden_search.json");
    let cases: Vec<SearchCase> =
        serde_json::from_str(raw).expect("golden_search.json parses into search cases");

    assert!(!cases.is_empty(), "golden_search.json must contain cases");

    let mut failures = Vec::new();
    for case in &cases {
        let got = forward_search(&ruleset, &case.query, &case.candidates);
        if got != case.expect {
            failures.push(format!(
                "case {:?}\n  expected: {:?}\n  got:      {:?}",
                case.name, case.expect, got
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "{} search golden case(s) failed:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

#[test]
fn embedded_ruleset_is_version_1() {
    assert_eq!(Ruleset::load_default().version, "1");
}
