# Routing Engine — Implementation Plan

**Status:** ready to execute
**Supersedes the sequencing in:** `2026-07-routing-engine-modularization.md` §6
**Rests on:** `2026-07-routing-engine-experiment-results.md` (twelve
experiments), `-module-design.md` rev. 2, `-design-rationale.md`

Every number here is measured, not estimated. Where something is still an
assumption it says so.

---

## 0. What the experiments changed

The plan that came out of analysis is not the plan that survived measurement.

| Original position | Measured | Consequence for implementation |
|---|---|---|
| "Ports cost <2%, monomorphise the hot loop" (bet #1, "the largest technical bet") | **Below resolution.** `Dem::sample` is 135 ns; dispatch is ~1 ns | **Drop the monomorphisation rule.** `Arc<dyn Heightfield>` throughout. `CostField` need not be generic. One fewer invariant forever |
| "Bit-exact cross-ISA parity is a risk" (A1, critical) | **Achieved.** Whole solver, both lanes, every hash identical x86_64 ↔ aarch64 | Parity contract stands. `libm::atanf` swap is cheap insurance, not a prerequisite |
| "Legacy deletion is one mechanical step" | **Cost half provably dead; feasibility half is behavioural** | **Split into two steps** with different risk profiles |
| "Per-request tuning via a `Tuning` struct" | **Rebuild is 555 ms–2.8 s** vs a 250 ms solve | `Arc<Index>` + `Params` + `rebind` is **mandatory**, not preferred |
| "Six Tobler copies — unify them" | **Two different physical models**, 41.9% apart on descent | Calibration change, own step, own baseline update |
| "Slope is double-counted, remove one" | Solver's term **structural**; contributor's term **additive** | Fix is known and tested; also a **12–24% DEM-work win** |
| "Contributor order may be part of the contract" (A8) | **No effect**, both lanes | Pack `CostSpec` can be an unordered set |
| "`turbo-geodata-memory` gets the corpus into CI" | Synthetic fixtures give unit coverage, not corpus coverage | Corpus-in-CI is a **data-distribution** problem needing a small real pack |
| Pack sizing ~380 MB / 100 km cell | **208 MB measured**, but DEM 3× cheaper and mask 32% of pack | Sizing table **withdrawn** pending 3–4 contrasting cells |

---

## 1. Defects to fix along the way

Found by measurement, independent of the refactor. Each needs an owner and
a gate.

| # | Defect | Evidence | Fix | Risk |
|---|---|---|---|---|
| **D1** | `point_covered` is `any(covers)`; advisory landcover grants routing coverage where no DEM reaches | E6, `tests/coverage_semantics.rs` | `Requirement::{Required, Advisory}`; coverage = intersection of Required | behavioural — routes that used to solve will honestly refuse |
| **D2** | Mesh slope charged twice; contributor's term reads a fixed **east–west** probe | E7b + E7c | Return 0 from `ToblerSlopeContributor::contribute` on `EdgeKind::Mesh` | **calibration** — routes move; also −12–24% DEM work |
| **D3** | Two different Tobler models (symmetric f32 mesh vs asymmetric f64 contributor), 41.9% apart on descent; guards differ 69.9° vs 76.1°, sentinels 10 000× | E7 | One pace curve in `cost::terrain`, one precision, one guard | **calibration** |
| **D4** | `f32::atan` differs across libm | E0 | `libm::atanf` (faster than std) | none — E1 shows it does not currently bite |
| **D5** | `dtm-bulk-load` shells to `raster2pgsql` with no preflight; missing binary → `exit 127` in a backtrace | dataset build | `which` check + clear message | none |
| **D6** | Graph build emits `subgraph_fragmented` and proceeds; `pgr_createTopology` without `pgr_nodeNetwork` leaves trails unnoded | artifact health output | fail the build, or run `pgr_nodeNetwork` | none |
| **D7** | `include_str!("../../../tools/cost-config.toml")` reaches outside the crate | static | move into the profile crate | none — blocks cdylib vendoring |
| **D8** | **`Dem::sample` is not a pure function of position.** In tile-overlap bands the answering tile depends on rstar order, so slicing changes elevations — and the national artifact is build-order dependent | E10b: slice ⊆ full, yet full returns `None` where slice returns a value (2 448 points) | de-overlap tiles at build time (preferred), or a canonical tie-break in `sample` | none to fix — **blocks packs** |

**D2 and D3 must not be bundled with the refactor.** They move geometry;
everything else in this plan must not. Mixing them destroys the only signal
that says a refactor step was clean.

---

## 2. The acceptance gate for every step

```
corpus geometry hash unchanged  AND  dem_cache_lookups unchanged
```

on **both lanes** (`--mode=off-trail` and `--mode=unified`). Phase 0 proved
these are exactly stable across runs while wall clock is not (16–19% range),
so:

> **Never gate a refactor step on timing.** Gate on hashes. Measure timing
> separately, with a microbenchmark, when timing is the question.

Two steps are exempt because they intentionally move geometry (D2, D3).
Those get a *reviewed baseline update* instead, with before/after route
comparison in `route-lab`.

Per-step checklist:

1. `cargo test -p turbo-tiles-pathfind` (includes `coverage_semantics`)
2. `eval-terrain --mode=off-trail` → hash + lookups unchanged
3. `eval-terrain --mode=unified` → hash + lookups unchanged
4. `tools/experiments/e11_conformance` still compiles
5. Boundary greps (§6) clean

---

## 3. Phases

Effort assumes one engineer. "Gate" is what must hold before the next step.

### Phase A — Foundations (2 weeks)

| A1 | **Fix the corpus-in-CI data problem.** Publish the Sjunkhatten pack (208 MB) to a fetchable location; make `tests/scenarios.rs` and `eval-terrain` run against it in CI. | 4 d |
| A2 | **D5, D6, D7** — the three no-risk defects. | 1 d |
| A3 | **`libm` swap (D4)** for `atan` on the routing path; add a build invariant forbidding fast-math/FMA contraction on the routing crates. | 1 d |
| A4 | **Re-measure pack sizing** across 3–4 contrasting cells (coastal, inland alpine, forested lowland) and replace the withdrawn table. | 2 d |

**Gate:** CI runs the corpus on every PR. This is what makes everything
after it safe, and it is currently the weakest link — a clean checkout today
executes zero scenarios.

### Phase B — Delete the dead cost model (1.5 weeks)

Split by risk, per E3.

| B1 | **Delete the legacy cost channel** — `compose_cell`, `compose_edge`, `CostLayer::cell_cost`'s multiplier, `LegacyLayerAdapter`, `layers.rs`, `vector_layers.rs`. | 4 d |
| B2 | **Port feasibility off legacy** — `point_covered`, `point_is_refused`, `endpoint_refused` onto contributors, **implementing D1 (Required/Advisory) at the same time** since the semantics change anyway. | 4 d |

**Gate B1:** hashes unchanged, both lanes. *Proven achievable* — E3 showed a
37× perturbation of the legacy multiplier moves nothing.
**Gate B2:** hashes may change *only* for routes whose endpoints lack Required
coverage; each such change reviewed and justified. `coverage_semantics.rs`
inverts to assert the new behaviour.

### Phase C — Ports (2 weeks)

| C1 | `turbo-route-model`: `Heightfield`, `ClassField`, `GeometrySet`, `TraversalNetwork`, `Requirement`, `ModeId`, domain types. **No generics needed** (E2). | 3 d |
| C2 | `turbo-geodata-artifacts` implements them over today's primitives. Swap the 12 `Dem` holders to `Arc<dyn Heightfield>`. | 3 d |
| C3 | `CostContributor` gains `rebind(&ParamSet)` and `fingerprint()`; split each contributor into `Arc<Index>` + `Params` (E4 — mandatory). | 3 d |
| C4 | Move `wgs84_to_utm33n` out of `turbo-tiles-elev` into `turbo-geo-frame` at L5. Engine becomes planar-only. | 1 d |

**Gate:** hashes unchanged, both lanes. Plus `e11_conformance` compiles
**with its local trait definitions replaced by imports** — that is the
acceptance criterion for "the ports step is done", and it is executable.

**Structural guard (the E7b lesson):** delete the concrete field types. If
`CostField` holds only `Arc<dyn Heightfield>` and it compiles, the concrete
path is gone by construction — a type-level proof, not an assertion.

### Phase D — Engine and composition (2 weeks)

| D1 | `trait Solver` + registry; `UnifiedAStar`, `FmmGradeLimited`, `NetworkDijkstra` as implementations. | 3 d |
| D2 | `turbo-route-engine`: `Engine::new(Terrain, CostModel, SolverSet, Budget)` — **takes values, no config, no I/O**. Planner stages. `RouteStrategy` with `PointToPoint` + `RoundTrip`. | 5 d |
| D3 | `turbo-route-compose` + `turbo-profile-no`: all config schemas, source resolution, preset merging, the inline constants from `routing_setup.rs` moved into TOML. | 3 d |

**Gate:** hashes unchanged, both lanes. Boundary greps clean.
`routing_setup.rs` deleted; `ROUTING_DEV_LOOP.md` updated, since it names
that file as the single wiring source of truth.

### Phase E — Calibration (1 week, gated separately)

**These move geometry on purpose.** One at a time, each with a reviewed
baseline update.

| E1 | **D3** — unify the pace curve on the asymmetric (published Tobler) model, one precision, one guard. | 2 d |
| E2 | **D2** — remove the contributor's mesh-side slope term. Expect −12% / −24% DEM work. | 2 d |
| E3 | Re-tune against the corpus; update `routing-baseline.json`. | 1 d |

**Gate:** corpus *quality* metrics (Fréchet vs ground truth) must not
regress; geometry hash changes are expected and reviewed in `route-lab`.

### Phase F — Packs and device (3 weeks)

| F0 | **Fix D8** — de-overlap DEM tiles at build time so `sample` is a pure function of position. Prerequisite: without it, pack parity cannot hold. | 2 d |
| F1 | `turbo-geodata-pack` + `turbo-route pack --bbox` (DEM tile filter, mask re-crop, vector AABB filter, graph CSR renumber, **1–2 km halo per E10**). | 8 d |
| F2 | `PyramidElevation` multi-resolution; re-measure pack sizes. | 2 d |
| F3 | `turbo-route-ffi` (uniffi over `Engine` + `compose`), `catch_unwind`, cargo-ndk — cloning the proven `turbomap-ffi` / `core/turbomap-android` pattern. | 5 d |

**Gate F3:** run `e1_crossisa` **on a real device** against bionic. E1
settled glibc-vs-glibc under QEMU; bionic on silicon is the one determinism
question still open.

### Phase G — Observability and lab (2 weeks, parallel from Phase D)

| G1 | `Observer` port; solver-agnostic events; `ndjson` sink for device capture. | 4 d |
| G2 | `inspect_corridor` per-cell attribution — `CostField::ensure` already computes it and discards it. | 2 d |
| G3 | Extract `apps/route-lab` from `PlotRoute.tsx`; add the attribution heatmap and A/B diff. | 4 d |

G2 is worth pulling early: it is what would have surfaced D2 years ago.

---

## 4. Timeline

```
A Foundations      ██████████                                    2 wk
B Delete legacy              ███████                           1.5 wk
C Ports                             ██████████                   2 wk
D Engine                                      ██████████         2 wk
E Calibration                                           █████     1 wk
F Packs + device                                             ███████████  3 wk
G Observability              ░░░░░░░░░░░░░░░░░░░░ (parallel)      2 wk
                   └────────────────────────────────────────────────────┘
                   0        2        4        6        8       10      12 wk
```

**Server-side modular and CI-gated: ~7.5 weeks (A–E).**
**Through device: ~12 weeks.**

Close to the original 7–9 / 6–8 estimate, but the *content* shifted:
foundations grew (corpus-in-CI is a real data problem), ports shrank (E2
removed the monomorphisation work), and a calibration phase appeared that
the original plan did not have.

---

## 5. What is proven vs still assumed

**Proven by measurement:**
- Port dispatch is free (E2)
- Cross-ISA parity holds end to end on glibc (E1)
- Legacy cost deletion cannot move geometry (E3)
- Contributor order is irrelevant (E5)
- Per-request rebuild is not viable (E4)
- The port API is coherent and ergonomic (E11)
- Config-as-string absorbs historical change (E9)

**Still assumed — flagged, not hidden:**
- **Bionic determinism on real hardware.** Only device testing settles it.
- ~~Pack slicing fidelity~~ — **measured (E10).** 500 m halo suffices;
  1–2 km recommended. But it surfaced **D8**, a format-level determinism
  defect that must be fixed before any pack ships.
- **Pack size across terrain types.** One coastal sample only.
- **That Phases B–D are hash-neutral.** Each step's gate is the test; E3
  makes B1 near-certain, C and D are mechanical but unproven.
- **Corpus coverage.** All conclusions rest on 12 hikes over one 100 km
  cell, not the 60-hike national corpus.

---

## 6. CI invariants to land with Phase C

```sh
# region-agnostic core
! grep -riE 'norway|n50|fkb|dnt|25833' crates/turbo-route-{model,cost,solvers,engine}/src

# no I/O or config below the composition layer
! grep -rE 'memmap2|zstd|reqwest|tokio|std::fs|std::env' crates/turbo-route-{model,cost,solvers}/Cargo.toml

# no paths in engine signatures (the rev-2 correction, mechanised)
! grep -rE 'fn .*&Path|fn .*PathBuf' crates/turbo-route-{model,cost,solvers,engine}/src

# adapters depend on the model only
```

Plus the runtime gates: solver conformance suite, `e11_conformance`
compiling against real imports, and the corpus hash check on both lanes.

The repo already enforces architecture with tests
(`test/architecture/feature_boundary_test.dart`), so this is consistent with
existing practice rather than a new discipline.

---

## 7. First three days

1. **Publish the Sjunkhatten pack** and wire it into CI (A1). Without it
   every gate in this plan is manual.
2. **Fix D5/D6/D7** (A2) — an afternoon, removes three papercuts.
3. **Run `eval-terrain` on the full national corpus** to confirm E3 and E5
   at 60 hikes rather than 12, before Phase B leans on them.

Step 3 is the one I would not skip. Every null in this document is measured
over one region; the deletion in Phase B is where a false null would cost
the most.
