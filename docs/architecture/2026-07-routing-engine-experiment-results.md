# Routing Engine — Experiment Results

Running log for the validation plan in
`2026-07-routing-engine-assumption-audit.md`. Each entry records what was
run, the raw result, and what it changes.

Tree state: `36d2038`. Harnesses live under the session scratchpad and are
reproducible from the code quoted in each entry.

---

## E7 — Do the six Tobler copies agree? **DONE. No. Two different models.**

**Method.** All seven call sites transcribed verbatim (four f32, three f64)
and evaluated over a shared slope range.

| Site | Precision | Form |
|---|---|---|
| A `fmm/tobler.rs:99` | f32 | `exp(-3.5·(\|g\|+0.05))` |
| B `fmm/tobler_aniso.rs:169` | f32 | `exp(-3.5·(\|g\|+0.05))` |
| C `fmm/elastica.rs:145` | f32 | `exp(-3.5·(\|g\|+0.05))` |
| D `unified.rs:96` | f32 | `exp(-3.5·(\|g\|+0.05))` |
| E `native_contributors.rs:187` | f64 | `exp(-3.5·\|g+0.05\|)`, g pre-`abs()`ed |
| F `native_contributors.rs:198` | f64 | `exp(-3.5·\|s+0.05\|)`, **signed** s |
| G `native_contributors.rs:829` | f64 | `exp(-3.5·\|s+0.05\|)`, **signed** s |

### Results

**1. The four f32 copies are bit-identical.** 39 996 samples over
`grad ∈ [-2, 2]`, zero mismatches. Duplication, but no drift. Same for
E vs D (0.000% over 0–60°) and F vs G (bit-identical).

**2. The mesh model and the contributor model are different physics.**

```
   slope       deg    D_sym s/m   F_asym s/m   abs diff     rel %
   -0.60     -31.0       5.8366       4.1130     1.7236     41.9%
   -0.30     -16.7       2.0425       1.4393     0.6032     41.9%
   -0.10      -5.7       1.0143       0.7147     0.2995     41.9%
   -0.05      -2.9       0.8514       0.6000     0.2514     41.9%
    0.00       0.0       0.7147       0.7147     0.0000      0.0%
   +0.30      16.7       2.0425       2.0425     0.0000      0.0%
   +0.60      31.0       5.8366       5.8366     0.0000      0.0%
```

- **A–E implement a *symmetric* model**: descent costs exactly what the
  equivalent ascent costs.
- **F–G implement *real* Tobler**: the minimum is at −0.05 (a 2.9° descent),
  where pace is 0.600 s/m.
- They agree exactly on flat and all ascent, and differ by a **constant
  41.9%** on all descent — the factor is `exp(3.5 × 2 × 0.05) = 1.419`.

**3. The refusal guards differ materially.**

| | threshold | trips at | sentinel |
|---|---|---|---|
| f32 (A–D) | `v < 1e-4` | \|grad\| 2.727 → **69.9°** | `1.0e6` |
| f64 (E–G) | `v <= 1e-6` | \|grad\| 4.043 → **76.1°** | `100.0` |

A 6.2° window where the mesh refuses and the contributor does not, and a
**10 000×** difference in the "impassable" sentinel.

### What it changes

Unifying the pace curve is **not** free cleanup. Collapsing A–E onto F–G
makes every descending mesh edge 41.9% cheaper, which will move routes.
It is a calibration change requiring a baseline update, and it must be
done as its own corpus-gated step — not folded into a refactor commit.

Which model is *correct* is a product question (Tobler's published curve is
the asymmetric one), but the answer is not "whichever is easier to merge".

---

## A12 — New finding: the contributor stack does not own mesh cost

Found while verifying E7. `unified.rs:469` prices a mesh edge as:

```rust
step_m * tobler_pace(grad) * mul * steep + gain
```

- `tobler_pace(grad)` — model **D** (symmetric f32), on `grad =
  ((b-a)/step_m).abs()`, i.e. the **direction of travel**.
- `mul = overlay.pace_mul(ni, nj)` — the `LazyCostField` multiplier, which
  runs the whole contributor stack *including* `ToblerSlopeContributor`
  (model **F**, asymmetric f64).
- `steep`, `gain` — two further slope terms hardcoded in the solver.

So **slope is priced twice on every mesh edge**, by two different models,
one of them outside the contributor composition entirely.

Worse, `cost_field.rs:92` builds the contributor's context as a **fixed
east–west** synthetic edge through the cell centre:

```rust
EdgeElevProbe::new(&self.dem, cx - 0.5 * cell_m, cy, cx + 0.5 * cell_m, cy)
```

so the contributor's slope reading depends on the arbitrary probe axis, not
on the direction of travel or the direction of steepest descent. An
east–west-trending slope and a north–south-trending slope of identical
steepness get different per-cell penalties.

**This is probably absorbed into the calibration** — the constants were
tuned with the double-count present — so it is not necessarily a live
routing bug. But two consequences are architectural:

1. **It falsifies a premise of my own module design.** I wrote that solvers
   price edges through `EffectiveCost`. They do not: the unified solver
   carries its own Tobler, steepness and gain terms outside the cost model.
   Any "one cost model" claim has to either move these terms into
   contributors (a calibration change) or state honestly that mesh base
   pace is solver-owned.
2. **It is exactly what `inspect_corridor` per-cell attribution would have
   surfaced**, and it went unnoticed for as long as attribution was
   discarded. That raises the priority of the attribution work from
   "cheapest large debugging win" to "the tool that finds this class of bug".

**Next step:** E7b — measure whether removing the double-count changes
corpus geometry, once artifacts exist. Until then this stays a *candidate*
defect, not a confirmed one.

---

## E0 — Does the float path reproduce across architectures? **DONE. Yes, with one exception, and the fix is cheap.**

**Method.** A digest harness hashing (FNV-1a over raw IEEE bits) every
transcendental the router calls, in the precisions it calls them, over 10⁶
samples each: `exp` (f32/f64), `tan`, `atan`, `powf`, `sqrt` (control), an
FMA-contraction probe, and the dot-product/normalise shape from
`DirectionalSlopeContributor`. Each function is evaluated twice — once
through `std`, once through the pure-Rust `libm` crate. Built for
`x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`, the latter run
under `qemu-aarch64-static` against cross-glibc.

### Result: 19 of 20 digests are bit-identical across architectures

```
17c17
< atan_f32_std     7ad6c32968633590     (x86_64)
> atan_f32_std     01515167c31be0c2     (aarch64)
```

**That is the only line that differs.** All four Tobler variants, `exp`
f32 and f64 (std *and* libm), `tan`, `powf`, `sqrt`, the FMA probe and the
dot-product shape all match bit-for-bit.

`atan_f32_libm` is **identical on both arches** — so the pure-Rust `libm`
crate is a working mitigation for the one function that diverges.

### Sizing the divergence (4 M samples, both arches)

| | x86_64 | aarch64 |
|---|---|---|
| `f32::atan` std vs libm mismatches | 16 850 (0.42%) | 15 907 (0.40%) |
| max ULP | 1 | 1 |
| cliff-refusal decision flips at 60° | **0** | **0** |

So each platform's `atanf` is within 1 ULP of the reference, and across 4 M
samples a 1-ULP difference never flipped the `grade_deg > CLIFF_DEG`
branch at `unified.rs:451`. Divergence would be *rare*, not systematic —
but rare is still fatal for hash-based verification, since one flipped tie
in one route breaks the digest.

### Cost of the fix

```
exp  f64   std  6.75 ns   libm 11.99 ns   -> 1.78x
exp  f32   std  4.78 ns   libm  9.82 ns   -> 2.05x
atan f32   std 10.26 ns   libm  6.70 ns   -> 0.65x   (libm is FASTER)
```

At ~250 k `exp` calls per solve, routing *everything* through `libm` costs
**+1.31 ms on a ~250 ms solve — +0.5%**. Swapping only `atan`, the one
function that actually diverges, is **free and slightly faster**.

### What it changes

**A1 is downgraded from critical to medium.** Bit-exact cross-ISA parity
is achievable and nearly free, so the pack-parity contract and the
geometry-hash verification strategy survive as designed.

Two caveats that keep this from being fully settled:

1. **This is glibc vs glibc under QEMU, not a device.** Android links
   **bionic**, a different libm implementation. The result establishes the
   *shape* of the problem (only `atan` among routing transcendentals) but
   does not settle bionic. The robust fix makes that moot: routing the
   float path through the `libm` crate removes the platform libm from the
   picture entirely.
2. **FMA contraction did not bite** — but only because rustc does not
   contract by default. This stays safe only as long as nobody enables
   `-C target-feature=+fma` or fast-math on the routing crates. Worth an
   explicit note in the build config.

**Action:** swap `f32::atan` → `libm::atanf` at `unified.rs:451` (free,
faster). Consider the full `libm` swap for the routing float path at +0.5%
before shipping on-device, and pin it as a build invariant.

---

## E9 — Would config-as-string have absorbed past changes? **DONE. 3 of 31 touched the contract, all additive.**

**Method.** Unshallowed the clone (60 → 820 commits) and took every commit
since 2026-01-01 touching `turbo-tiles-pathfind`, `turbo-tiles-fmm`,
`routing_setup.rs`, or `cost-config.toml`. 31 commits. Each classified by
what it would have been under the proposed design.

| Class | Count | Would it change the FFI contract? |
|---|---|---|
| Internal / orchestration (perf, refactor, caching, repair) | 12 | no |
| Hygiene, tests, tooling | 7 | no |
| Contributor or solver addition | 4 | no |
| Config / compose only | 3 | no |
| **Request-contract change** | **3** | **additive only** |
| Mixed config + solver | 2 | no |

### The three that touched the contract

| Commit | Added |
|---|---|
| `4768f5de` multi-waypoint routing | `points`, `from`, `to` (all `Option`) |
| `cf6f6e0b` trip presets | `preset: Option<String>` |
| `a324e84f` avoid-marked + round-trip | `avoid`, `avoid_radius_m`, `round_trip` |

**Every one is a new optional field.** Zero breaking changes in 31 commits
and roughly seven months of routing development.

### What it changes

Bet #3 holds, and the measurement sharpens *why* the design passes
`request_json: String` rather than a typed uniffi record:

- Under a **typed** FFI record, those 3 commits each force binding
  regeneration and a coordinated app release — ~10% of routing commits.
- Under **JSON-in-string**, they need **zero** binding changes, because
  additive optional fields are backward-compatible on both sides.

So the string boundary is not laziness; it converts 3 forced app releases
into 0. That is the concrete payoff, and it is now a number rather than an
argument.

### Bonus evidence from the same history

`c5bd3e7c` — *"per-request cost overrides now reach the unified solver"* —
is a shipped bug of exactly the class the single `overlay` stage prevents:
the cost patch was resolved separately inside each solver, so overrides
silently applied on one path and not the other. The module design's
"resolve once, in one place" is not a hypothetical improvement; it fixes a
defect this codebase has already paid for.

Two further commits (`df063bb6` unified router, `36313e67` FMM) added whole
solvers, and four (`7a44a724`, `842ed234`, and two mixed) added or retuned
contributors — none of which would touch the contract under the `Solver` /
`CostContributor` registries.

---

## E6 — What does `point_covered` actually claim? **DONE. Confirmed: advisory layers grant coverage.**

**Method.** A real test, `crates/turbo-tiles-pathfind/tests/coverage_semantics.rs`,
built on the existing synthetic-artifact scaffolding — no Kartverket data
needed. A 2.56 km DEM tile and a 20 km landcover mask sharing a corner, with
a probe point 15 km east: far outside the DEM, still inside the mask.

```
running 3 tests
test dem_alone_reports_coverage_only_inside_the_dem ... ok
test required_vs_advisory_is_not_expressible_today ... ok
test advisory_landcover_layer_grants_coverage_with_no_elevation_data ... ok
```

### Result

With only a DEM loaded, the probe point is correctly **not** covered. Add a
`LandcoverLayer` over the forest mask — registered exactly as
`routing_setup.rs` does it — and `point_covered()` returns **true**, while
`dem.sample()` at the same point returns `None`.

So the pre-check that exists to stop the solver "building a uniform-cost
mesh and returning a straight line — semantically a lie" (its own comment)
can be satisfied by a layer that knows only whether there is forest there.

### Why: the layers disagree about what `covers` means

| Layer | `covers` | Intent |
|---|---|---|
| `SlopeLayer`, `AvalancheTerrainLayer` | `dem.sample().is_ok()` | authoritative |
| `MaskRefusalLayer` | `mask.refused().is_ok()` | authoritative |
| `LandcoverLayer` | `mask.refused().is_ok()` | **advisory** |
| `TrailProximityLayer` | `any_near(x, y)` | **deliberately narrowed** |

`TrailProximityLayer` already carries the comment: *"Proximity is a bias,
not a coverage claim. It returning true would short-circuit the no-terrain-
data precheck."* **The Required/Advisory distinction is already understood
in this codebase** — it is just enforced by hand, one layer at a time, in
prose. `LandcoverLayer` never got the same treatment.

### What it changes

A3 is confirmed and its fix is now specified by a failing-if-regressed
test. `Requirement::{Required, Advisory}` on the layer/contributor trait,
with coverage as the **intersection of Required layers**, is not new design
— it is generalising a rule two layers already follow informally.

Severity in practice is bounded: this only bites where a landcover or
vector mask extends past the DEM. Norway's masks are built from the same
national footprint, so the overlap is small today — but region packs will
routinely have mismatched per-layer extents, which is exactly when it
starts mattering.

---

## Environment notes

- `rustc 1.94.1`, x86_64-unknown-linux-gnu, single target installed.
- No `~/turbo-artifacts`; `TILESERVER_ARTIFACT_DIR` unset. Experiments
  needing real data (E2, E3, E4, E5, E6, E10) are blocked on the
  Sjunkhatten dataset build.
- Docker daemon started manually (`sudo dockerd`); `pgrouting/pgrouting`
  tag `16-3.4-3.6` does not exist — image selection pending.
- aarch64 target not installed, so E1 needs `rustup target add` plus the
  NDK toolchain.
