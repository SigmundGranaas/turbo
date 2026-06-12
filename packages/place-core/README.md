# place-core

The single source of truth for Turbo's search and reverse-geocoding **decision**
logic. Pure Rust, no I/O — each platform does its own data access and funnels
candidates into the shared ranker.

See `docs/architecture/2026-06-native-search-geocoding-plan.md` for the full plan.
This crate is **Phase 0**: the pure logic + ruleset + golden fixture. FFI bindings
(Dart via `flutter_rust_bridge`, Kotlin/Swift via UniFFI, .NET via P/Invoke) and the
embedded SQLite engine land in later phases.

## What's here

| File | Role |
|------|------|
| `src/model.rs` | Domain types (`Candidate`, `LocationDescription`, `Tier`, `Qualifier`, …) mirroring the clients' `LocationDescription`. |
| `src/ruleset.rs` | The versioned, data-driven tuning: kind groups, distance bands, penalties. |
| `src/classify.rs` | `classify(kind, meters)` → `(tier, qualifier)` — port of the Flutter `categorizeFeature`. |
| `src/rank.rs` | `reverse_geocode(input)` → the cascade (tight toponym → park → loose toponym → address → kommune) + dedup + enrichment. |
| `src/geo.rs` | `haversine_m` for callers that only have coordinates. |
| `ruleset.v1.json` | The embedded ruleset (also served at `GET /api/places/ruleset/{version}`). |
| `golden.json` | The behavioural contract — the 28+ invariants ported from the Flutter/Android test suites. |

## Design

- **Policy is data + code, not either/or.** The *algorithm* (cascade, dedup,
  classification order) lives in this crate and is shared by every runtime. The
  *numbers* (distance caps, penalties, kind sets) live in `ruleset.v1.json` and
  ship without recompiling the native library.
- **Canonical qualifiers** are the richer Flutter set (`on / closeTo / atPlace /
  inArea / near`). The Android binding folds `closeTo`→Near and `inArea`→In to
  match its 4-value `PlaceQualifier`.
- **`golden.json` is the contract.** `cargo test` runs it here; each FFI binding
  later smoke-tests a subset so the bindings can't drift from the core.

## Develop

```sh
cargo test          # unit tests + golden fixture
cargo clippy --all-targets
cargo fmt
```

To extend behaviour: add/adjust a rule in `ruleset.v1.json`, add a case to
`golden.json`, and keep `cargo test` green. Touch `rank.rs` only for genuinely new
*structure* (a new cascade step), not for tuning.
