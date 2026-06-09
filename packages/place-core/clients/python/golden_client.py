#!/usr/bin/env python3
"""Synthetic UniFFI test client (Python).

Drives `place-core` through the *generated UniFFI binding* — not the Rust API —
and replays the same `golden.json` / `golden_search.json` fixtures the Rust
tests use. If this passes, the FFI surface (records, enums, the PlaceEngine
object) lifts/lowers correctly, so we can trust the bindings before wiring them
into the apps.

Run via ../../clients/run_python_client.sh (it copies the cdylib next to the
generated module first).
"""

import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
CRATE = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path.insert(0, HERE)

import place_core as pc  # noqa: E402  (must follow sys.path tweak + cdylib copy)

_QUALIFIER = {
    "on": pc.Qualifier.ON,
    "closeTo": pc.Qualifier.CLOSE_TO,
    "atPlace": pc.Qualifier.AT_PLACE,
    "inArea": pc.Qualifier.IN_AREA,
    "near": pc.Qualifier.NEAR,
}


def _candidate(d):
    return pc.Candidate(
        name=d["name"],
        kind=d.get("kind", ""),
        distance_m=float(d["distance_m"]),
        status=d.get("status"),
        secondary=d.get("secondary"),
    )


def _reverse_input(d):
    pa = d.get("protected_area")
    addr = d.get("address")
    komm = d.get("kommune")
    return pc.ReverseInput(
        toponyms=[_candidate(c) for c in d.get("toponyms", [])],
        protected_area=pc.ProtectedArea(name=pa["name"], kind=pa.get("kind")) if pa else None,
        address=pc.Address(text=addr["text"], secondary=addr.get("secondary")) if addr else None,
        kommune=pc.Kommune(name=komm["name"], fylke=komm.get("fylke")) if komm else None,
        elevation_m=d.get("elevation_m"),
    )


def _expected_description(d):
    if d is None:
        return None
    q = d.get("qualifier")
    return pc.LocationDescription(
        title=d["title"],
        qualifier=_QUALIFIER[q] if q else None,
        secondary=d.get("secondary"),
        kommune=d.get("kommune"),
        fylke=d.get("fylke"),
        distance_m=d.get("distance_m"),
        elevation_m=d.get("elevation_m"),
    )


def _search_candidate(d):
    return pc.SearchCandidate(
        name=d["name"],
        kind=d.get("kind", ""),
        distance_m=d.get("distance_m"),
        description=d.get("description"),
    )


def _expected_hit(d):
    return pc.SearchHit(
        index=d["index"],
        title=d["title"],
        description=d.get("description"),
        icon=d["icon"],
    )


def _load(name):
    with open(os.path.join(CRATE, name), encoding="utf-8") as f:
        return json.load(f)


def main():
    engine = pc.PlaceEngine.with_default_ruleset()
    assert engine.ruleset_version() == "1", "binding reached an unexpected ruleset"

    failures = []

    for case in _load("golden.json"):
        got = engine.reverse_geocode(_reverse_input(case["input"]))
        want = _expected_description(case["expect"])
        if got != want:
            failures.append(f"[reverse] {case['name']!r}\n    want: {want}\n    got:  {got}")

    for case in _load("golden_search.json"):
        got = engine.forward_search(case["query"], [_search_candidate(c) for c in case["candidates"]])
        want = [_expected_hit(h) for h in case["expect"]]
        if got != want:
            failures.append(f"[search]  {case['name']!r}\n    want: {want}\n    got:  {got}")

    if failures:
        print(f"FAIL — {len(failures)} case(s) diverged across the FFI boundary:\n")
        print("\n\n".join(failures))
        return 1

    print("OK — golden fixtures pass through the UniFFI Python binding")
    return 0


if __name__ == "__main__":
    sys.exit(main())
