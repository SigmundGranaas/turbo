# Routing Engine — Experiment Results

> **The harnesses are gone; the results are the point.** Five of the six
> experiment crates under `tools/experiments/` were deleted once they had
> answered their question — keeping a scaffold after the building is up.
> `e1_crossisa` survives because bionic-on-silicon is still unverified.
> What each deleted harness settled is tabulated in
> `apps/tileserver/tools/experiments/README.md`, and in full below.


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

## E4 — How expensive is contributor construction? **DONE. Rebuild-per-request is dead.**

**Method.** `crates/turbo-tiles-pathfind/tests/contributor_construction_cost.rs`
— synthetic grid graphs at four scales with `fkb_type` cycling over
sti/vei/skiloype so all three R-trees populate. Times
`TrailProximityContributor::new` in isolation and the whole
`Pathfinder::with_defaults` stack. No artifacts needed.

```
   side     nodes      edges   TrailProx::new  with_defaults (full)
     20       400       1520          0.53 ms            1.15 ms
     60      3600      14160          8.75 ms           15.44 ms
    120     14400      57120         30.73 ms           53.86 ms
    200     40000     159200         88.42 ms          149.30 ms

per-edge construction cost: 0.555 us/edge
  extrapolated to 1 000 000 edges:   555 ms
  extrapolated to 5 000 000 edges:  2777 ms
```

Scaling is linear in edges. `TrailProximityContributor` is ~60% of total
construction cost.

### What it changes

Against a **250 ms mean solve**, rebuilding the cost model per request on a
national graph costs **555 ms to 2.8 s** — two to eleven times the entire
solve. That conclusively refutes the "just rebuild the `CostModel` per
request" alternative and makes the **`Arc<Index>` + `Params` split
mandatory**, exactly as A2 proposed.

E11 measures the other side of the split: `rebind` costs **0.098 µs per
contributor** — an Arc clone plus a scalar write. The ratio between rebuild
and rebind at national scale is ~5.6 million to one. This is not a close
call, and the split point is now chosen on evidence rather than instinct.

---

## E11 — Portability conformance **DONE. The proposed API compiles and holds up.**

**Method.** `tools/experiments/e11_conformance` — a standalone crate that
declares the proposed L1 shapes (`Heightfield`, `CostContributor`,
`Requirement`, `ModeId`), a typed `CostModel::builder()`, and an `Engine`
whose constructor takes values, then drives it from a procedurally
generated in-memory heightfield.

```
extent      2048 x 2048 m
across ridge    3080.7 s over 451 pts
along valley    1210.6 s over 401 pts
rebind           0.098 us/contributor
fingerprint   c6be72420266ac44

PASS — no file, no config string, no pack, no profile, no CRS.
```

The route across the ridge costs 2.5× the route along the valley, an
out-of-extent query returns `OutsideExtent` rather than a straight line
(the E6 guarantee), and the whole thing is driven from a `Vec<f32>`.

### Why this is worth having as a compiling artifact

**This is the check that would have caught the rev. 1 error.** Writing
`Engine::open(config, pack_dir)` in this file immediately raises "what path
does a game engine pass?" — which is the entire finding. A design claim
written in prose survives being wrong; one written as a compiling program
does not.

It also validated three audit fixes as *ergonomic*, not just correct:
`Requirement::Required` on `ToblerSlope`, `rebind` as an Arc clone, and
`fingerprint` for the leg-cache key all read naturally in the builder.

### The gap it gates

Against today's tree the block is impossible to write: `Dem` exposes only
`open(&Path)` / `open_with_cache(&Path, usize)` with no in-memory
constructor; `Pathfinder::with_defaults` takes `Option<Arc<Dem>>`, the
concrete artifact type; and `wgs84_to_utm33n` lives inside
`turbo-tiles-elev`. **When the local trait definitions in this file can be
replaced by imports from the real engine and it still compiles, the ports
step is done.** That is the acceptance criterion, and it is executable.

---

## E3 — Is the legacy `CostLayer` stack geometry-neutral? **DONE. Its cost channel is provably dead.**

**Method.** Three runs of `eval-terrain` over the 12 `nordland` corpus hikes
against the Sjunkhatten artifacts, comparing `corpus_hash` (a digest over
every route's geometry).

| Run | Change | `corpus_hash` | `dem_lookups` |
|---|---|---|---|
| baseline | none | `4558525db8df425c` | 1 921 389 |
| **E3 probe** | legacy `SlopeLayer` multiplier → `m * 37.0 + 11.0` | **`4558525db8df425c`** | **1 921 389** |
| **positive control** | native `ToblerSlopeContributor` pace × 1.01 | `750c927e4a16fe09` | 1 924 803 |

### The control is what makes this meaningful

A null result is worthless unless the instrument can detect a real change.
**A 1% perturbation of the native contributor moves the hash and the DEM
lookup count.** A **37×** perturbation of the legacy layer moves neither —
not one route, not one DEM sample.

So the legacy cost channel is not merely unused in practice; it is not
evaluated at all on the solve path.

### But "legacy is inspect-only" is wrong, and that matters for the plan

Static analysis of every `self.layers` read in `pathfinder.rs` shows the
legacy stack is still load-bearing for **three behaviours**, none of which
is diagnostic:

| Site | Use | Kind |
|---|---|---|
| `:852` `point_covered` | feasibility pre-check | **behavioural** |
| `:911` `point_is_refused` | endpoint repair / snap-out | **behavioural** |
| `:930` `endpoint_refused` | endpoint refusal error | **behavioural** |
| `:1007`, `:1022` | breakdown + inspect | diagnostic |

The audit (and the modularization doc) described the legacy stack as
serving "the inspect endpoint and a build-time refusal sampler". That
understates it: **coverage and endpoint refusal both run off legacy
layers**, and E6 already showed `point_covered`'s semantics are wrong.

### What it changes

Bet #2 splits cleanly in two:

- **Cost half: zero risk.** Deleting the multiplicative machinery
  (`compose_cell`, `compose_edge`, `CostLayer::cell_cost`'s multiplier,
  `LegacyLayerAdapter`) cannot move geometry. Measured, not argued.
- **Feasibility half: a real port.** `point_covered`, `point_is_refused`
  and `endpoint_refused` must be reimplemented against contributors —
  and reimplemented *correctly*, since E6 showed the current `.any(covers)`
  semantics is a defect being carried forward.

The 1.5-week estimate for the deletion step stands, but it is now
"1 week of safe deletion plus a small, behaviour-changing port that needs
its own corpus gate", rather than one uniform mechanical change. That
reframing is exactly what an hour of measurement was supposed to buy.

**Scope caveat:** 12 hikes over one 100 km cell, not the 60-hike national
corpus. Sufficient to establish sensitivity (the control fires) and to
falsify the cost-channel hypothesis, but a full-corpus confirmation should
run before the deletion lands.

---

## E5 — Does contributor order change geometry? **DONE. No.**

**Method.** `natives.reverse()` immediately before `Pathfinder` construction
in `with_defaults_and_config` — same contributor set, same vetoes, only the
order of the additive summation, the `pace_factor` product, and the
veto short-circuit changes. Same 12-hike eval.

| Run | `corpus_hash` |
|---|---|
| baseline | `4558525db8df425c` |
| **E5 — stack reversed** | **`4558525db8df425c`** |

Identical. And E3's control proves the instrument detects a 1% cost change,
so this is a real null, not an insensitive measurement.

### What it changes

**A8 is downgraded.** Floating-point addition is not associative, so
reordering *can* in principle move a route; on this corpus it does not.
The contributions evidently differ enough in magnitude that reordering
never crosses a comparison boundary in the priority queue.

Practical consequences:

- **The pack format does not need to pin contributor order** for
  determinism. One less thing in the contract.
- **Order still matters for diagnostics**: `compose_edge_walk_seconds`
  returns on the *first* veto, so the reported `vetoed_by` label depends on
  stack order even though the refusal outcome does not. Worth keeping
  stable for reproducible debugging, but it is a UX property, not a
  numerical one.
- The `CostSpec` in a pack can therefore be an unordered set of rows.

**Same scope caveat as E3:** 12 hikes, one cell. A null here does not prove
order-independence globally — it shows the effect is not large enough to
bite on a realistic sample, which is what the question was actually asking.

---

## E7b — Does removing the slope double-count move geometry? **First attempt INVALID. Rerun in progress.**

### The error, recorded because it is instructive

The probe replaced `unified.rs:469`'s `step_m * tobler_pace(grad) * mul *
steep + gain` with `step_m * base_pace_s_per_m * mul * steep + gain` and
the corpus hash came back **unchanged** — which would have been a
remarkable result: deleting the solver's entire slope term, no effect.

It is not remarkable, it is meaningless. `eval-terrain` defaults to
`--mode=off-trail`, which sets `force_off_trail: true`, and
`Pathfinder::solve_inner` dispatches that to `solve_off_trail` → the **FMM**
solver. `unified.rs` never executes in that mode. **The patch was in code
the harness does not run.**

The build even said so — `warning: function tobler_pace is never used` —
and that warning was read as confirmation the patch had landed, rather than
as the hint that the function had no live callers in the exercised path.

Rerunning with `--mode=unified`, which the flag's own help text describes as
"the unified A* users hit".

### The caveat this puts on E3 and E5

Both were measured in the **default off-trail FMM lane**, not the unified
lane that serves production traffic. Their conclusions still hold, for
reasons that are structural rather than lucky:

- **E3**: the legacy `CostLayer` cost channel is unread by *either* lane —
  the three live `self.layers` sites are all `Pathfinder`-level
  (coverage, endpoint refusal), which run before lane dispatch and are
  therefore common to both.
- **E5**: contributors reach both lanes through the shared
  `LazyCostField`, so a reordering null in one lane is evidence for the
  other, though not proof.

Still, both should be re-run with `--mode=unified` before the deletion
step lands. A result measured on a lane users do not hit is weaker evidence
than it looks, and this experiment is a reminder of exactly that.

### Rerun on `--mode=unified`: the two slope terms are NOT interchangeable

| Run | lane | `corpus_hash` | mean solve | DEM lookups |
|---|---|---|---|---|
| baseline | off-trail (FMM) | `4558525db8df425c` | 749 ms | 1 921 389 |
| baseline | **unified (production)** | `0575d4fd66c591a4` | **22.5 ms** (p50 3.2, max 109) | 249 976 |
| **E7b probe** | unified | **did not complete** | **>25 min for 12 hikes** | — |

Removing `tobler_pace(grad)` from `unified.rs:469` does not merely change
the route — **it makes the search explode**. The baseline solves all twelve
hikes in 0.27 s total; the probe had not finished a single pass after
twenty-five minutes, a slowdown of >5000×. Killed and reverted.

### This corrects A12

The original finding said slope is "priced twice … by two different models,
one of them outside the contributor composition entirely", implying one is
redundant. That is wrong in an important way:

- **`tobler_pace(grad)` is *directional*** — `grad` comes from the actual
  elevation delta between the two cells being traversed.
- **`ToblerSlopeContributor` is *isotropic per cell*** — `cost_field.rs:92`
  evaluates it on a fixed **east–west** synthetic edge through the cell
  centre, so it yields one number per cell regardless of travel direction.

Delete the directional term and every exit from a cell costs the same. The
A\* loses its terrain-following gradient entirely and floods the corridor —
exactly the observed blowup.

**Corrected statement of A12:** slope is double-counted in *magnitude* but
priced only once *directionally*. The two terms are not duplicates and the
solver's term is load-bearing.

The underlying defect stands, and is now better characterised:

1. The contributor's slope reading depends on an **arbitrary axis**
   (east–west), so an east–west-trending slope and a north–south one of
   identical steepness get different per-cell penalties.
2. The magnitudes **do** compound, and they compound across the two
   *different physical models* E7 identified (symmetric f32 × asymmetric
   f64).

But the fix is not "delete the solver's term". The untested direction —
and the experiment that would actually test redundancy — is removing the
**contributor's** slope term while keeping the solver's directional one.
That is E7c, and it is the one worth running.

### The general lesson

This is the same failure shape as the rev. 1 composition-root error and
finding A4: **a change applied to one path while the conclusion was drawn
about the system.** The guard is cheap and was skipped — confirm the probe
executes before trusting the measurement, e.g. by asserting the patched
branch is reached, or by checking a *positive control on the same lane*
(which is precisely what made E3 trustworthy and what E7b lacked).

---

## Sjunkhatten test dataset — built, and it corrects the pack-size estimate

A real, reproducible regional dataset, entirely from Kartverket. This is the
first concrete instance of the "region pack" the offline plan depends on.

### Recipe

```sh
docker run -d --name turbo-tiles-db-test -e POSTGRES_PASSWORD=testpass \
  -p 55433:5432 pgrouting/pgrouting:16-3.5-3.7
psql … -c "CREATE EXTENSION postgis; postgis_raster; pgrouting; pg_trgm"
tileserver migrate

# N50 Nordland — via the repo's own Geonorge client
tileserver ingest --job=provision-n50 --area=18          # 883 960 rows

# DTM10 UTM33, map cell 7405 (the 100 km cell over Sjunkhatten).
# Dataset UUID dddbb667-1303-4ac5-8640-7ec04c0e3918, areas "7405-1".."7405-4",
# format TIFF, projection 25833. POST /api/order, then GET each downloadUrl
# WITH REDIRECTS FOLLOWED (curl -L; without it you get 0-byte files).
apt-get install -y postgis                                # raster2pgsql
tileserver ingest --job=dtm-bulk-load --file=<zip> --source=dtm10   # x4

psql -c "SELECT pgr_createTopology('paths.edge',0.0001,'geom','id',
                                   'source_node','target_node')"
tileserver build-artifacts --kind={dem,graph,mask} --out=…
```

### What it produced

| | |
|---|---|
| Edges | 78 537 — **12 188 sti, 9 695 traktorvei, 56 654 vei** |
| Topology | 70 322 vertices, all edges noded |
| DEM | 8584 × 10120 cells @ 10 m, 1360 tiles, 0 absent |
| Mask | 13411 × 20707 cells, 73.3 M water, 1.3 M glacier |
| Graph artifact | 157 074 directed edges |

### Artifact sizes — the correction

For one 100 × 100 km cell (10 000 km²):

| Artifact | Size | Per km² |
|---|---|---|
| `norway.dem` | 112.9 MB | 11.3 KB |
| `norway.mask` | 66.2 MB | 6.6 KB |
| `norway.graph_geom` | 21.4 MB | 2.1 KB |
| `norway.graph` | 8.0 MB | 0.8 KB |
| **total** | **208 MB** | **20.8 KB** |

The on-device analysis estimated **~34 KB/km² for the DEM alone**,
extrapolated from "11 GiB over ~324 000 km²", and ~380 MB for a 100 × 100 km
region. The measured total is **208 MB — 1.8× smaller** than estimated.

But the *composition* is wrong in both directions, which matters more than
the total:

- **DEM is 3× cheaper than estimated** (11.3 vs 34 KB/km²).
- **Mask is far more expensive than assumed.** The earlier analysis lumped
  "everything else" at ~10 MB for 2500 km² (≈4 KB/km²); the mask alone is
  6.6 KB/km², and at 66 MB it is 32% of the pack.

**Caveat, and it is a large one:** Sjunkhatten is coastal. This cell is
substantially fjord and open sea, where DEM nodata compresses almost to
nothing while the water mask is dense. An inland cell would invert the
ratio. Treat these as one sample, not a calibration — the honest conclusion
is that **per-km² pack cost varies enough with terrain type that the sizing
table in the on-device analysis should be replaced by measurements over
three or four contrasting cells** before anyone commits to a download-size
promise.

### Two incidental defects found while building

1. **`dtm-bulk-load` shells out to `raster2pgsql` with no preflight check.**
   Missing binary surfaces as `exit status: 127` inside a Rust backtrace.
   A one-line `which` check with a clear message would save the next person
   the same detour.
2. **The graph health check flags a real data problem the build does not
   fail on:** `fkb_type=1 subgraph: 3753 components, largest 1.1%`. Running
   `pgr_createTopology` without `pgr_nodeNetwork` first leaves trails
   crossing without being noded. The warning is emitted and ignored. For
   this experiment it is acceptable (the mesh path is what E3/E5/E7b
   exercise), but it means **trail-following quality on this dataset is not
   representative**, and any conclusion about trail routing from it would be
   invalid.

---

## E2 — What does the elevation port cost? **DONE. Below measurement resolution.**

### Phase 0 first: the corpus cannot resolve 2%

Five identical runs per lane on unmodified code:

| lane | mean | sd | range | geometry hash | dem lookups |
|---|---|---|---|---|---|
| off-trail | 588.17 ms | 43.0 (**7.31%**) | 111.6 (**18.97%**) | 1 unique | 1 unique |
| unified | 18.05 ms | 1.10 (**6.08%**) | 2.89 (**15.99%**) | 1 unique | 1 unique |

**A 2% effect is invisible in 16–19% spread.** The harness is an excellent
*determinism* instrument — geometry hash and DEM lookup count are perfectly
stable across all ten runs — and a poor *timing* one, on this shared,
virtualised host with only 12 hikes and a 3.6 s max dominating the mean.

**This retroactively invalidates the single-run timings quoted earlier in
this document.** The off-trail "baseline" recorded as 749.3 / 749.7 ms was
measured while builds ran concurrently; the clean N=5 range is 544.8–656.4 ms.
Every wall-clock number from a single run in this file should be read as
±20%. The *hashes* and *lookup counts* are unaffected — those were stable —
so E3, E5 and E7b's conclusions stand.

### Phase 1: measure dispatch directly, then multiply

`tools/experiments/e2_dispatch` opens the **real** Sjunkhatten DEM and walks
2 M corridor-ordered points (matching the row-major access `CostField` and
`DemElevation` produce — a random scatter would exaggerate tile-cache cost
and mask dispatch), min-of-7 reps, cache pre-warmed.

```
sample():
  A concrete                    134.826 ns/call
  B dyn                         134.313 ns/call
  C generic (monomorphised)     134.094 ns/call

slope_aspect():
  A concrete                    224.837 ns/call
  B dyn                         225.161 ns/call

per-call deltas
  sample  B-A = -0.514 ns  (-0.38%)
  sample  C-A = -0.732 ns  (-0.54%)
  slope   B-A = +0.324 ns  (+0.14%)
```

**The deltas are inconsistent in sign** — `dyn` measures marginally *faster*
for `sample` and marginally *slower* for `slope_aspect`. That is the
signature of an effect below the bench's own resolution (≈±0.5 ns at this
sample count), not of a real difference. The honest statement is
**|delta| < 1 ns, i.e. under 0.75% of one call.**

### Derived solve-level penalty

| lane | lookups | total solve | dyn penalty | % of solve |
|---|---|---|---|---|
| off-trail | 1 921 389 | 7 058 ms | −0.99 ms | **−0.014%** |
| unified | 249 976 | 217 ms | −0.13 ms | **−0.059%** |

**Indistinguishable from zero, against a 2% budget.**

### Why the prediction was conservative

The plan predicted 0.04–0.2%, assuming ~2 ns dispatch. Measured: nothing
detectable — because **`Dem::sample` costs 135 ns**, dominated by the rstar
tile lookup and cache access. A vtable indirect call is ~1% of that.
`slope_aspect` at 225 ns is even more dispatch-insensitive.

The premise "ports are cheap" is not merely satisfied, it was never close to
being at risk: the per-call work is ~70–100× the dispatch cost.

### What it changes — the design simplifies

This is the **row-1 outcome** from the E2 plan: both arms pass, so

> **the "generics in the hot loop, `dyn` at the edges" rule is optional.**

`Arc<dyn Heightfield>` can be used throughout. The rationale's §4.1 —
"the design's largest technical bet", with the monomorphization discipline
as its mitigation — resolves to *no bet at all* for the elevation port. One
rule fewer to enforce forever, and `CostField` need not be generic.

Bet #1 in the rationale is **confirmed**, and its fallback (enum dispatch,
monomorphising over a concrete `Sources` parameter) is not needed.

### The caveat that remains

This measures dispatch **in isolation**. It cannot see two second-order
effects:

1. **Lost inlining cascading** into the callers' surrounding arithmetic.
2. **Icache pressure** from monomorphised code bloat — which would favour
   `dyn`, not penalise it.

The margin absorbs them: at <0.02% of a solve, even a 10× underestimate
lands at 0.2%, still an order of magnitude under budget. If the real swap
is ever suspected of costing more, the instrument is `perf stat` instruction
counts — **not** corpus wall clock, which phase 0 proved cannot see it.

---

## E1 — Is the whole solver bit-reproducible across ISAs? **DONE. Yes. Every hash identical.**

**Method.** `tools/experiments/e1_crossisa` — a standalone harness that opens
the real Sjunkhatten artifacts, builds a `Pathfinder` with the default layer
stack, and solves six corpus routes on **both lanes**, hashing each route's
geometry. Cross-compiled to `aarch64-unknown-linux-gnu` and run under
`qemu-aarch64-static`.

Deliberately standalone rather than cross-compiling `tileserver`: the server
binary drags in sqlx, rustls and axum, whose C dependencies are a yak-shave
unrelated to the question. The solver crates are pure Rust apart from zstd.

### Result

```
$ diff <(e1_x86) <(e1_aarch64)
IDENTICAL — every route, both lanes, including the error case
```

| lane | corpus hash (x86_64) | corpus hash (aarch64) |
|---|---|---|
| off-trail | `a5b78091e510c5ee` | `a5b78091e510c5ee` |
| unified | `a0d848ef98fa8bd6` | `a0d848ef98fa8bd6` |

Every per-route hash, every point count, every length to the digit — and
the one route that fails (`EndpointRefused` on `mask_refusal`) fails
identically on both.

### Why this is much stronger than E0

E0 was a *necessary* condition: of every transcendental on the routing path,
only `f32::atan` differed between the two libms. That left the *sufficient*
question open — a single flipped comparison anywhere in a priority queue can
fork a route, and a solve makes millions of them.

E1 closes it end to end. Millions of float operations, thousands of queue
comparisons and tie-breaks, two independent solver families, and the output
is bit-identical. **The `f32::atan` divergence E0 found does not bite in
practice**, consistent with its own finding of zero cliff-refusal flips in
4 M samples.

**Bit-exact cross-ISA parity is achieved, not merely achievable.** Bet #6
and finding A1 resolve in the design's favour.

### Caveats that survive

1. **glibc vs glibc, under QEMU.** Android links **bionic**. This settles
   the *ISA* question, not the *platform* one. The `libm` swap for `atan`
   remains cheap insurance and should still be taken.
2. QEMU is IEEE-accurate for these operations, but it is emulation, not
   silicon.
3. Six routes over one region.

The residual risk is now narrow and specific: bionic's libm on real
hardware. That is a device test, not an architecture question.

---

## E7c — Is the contributor's slope term the redundant one? **DONE. It changes routes but is not structural.**

A12's untested direction. E7b removed the solver's *directional* Tobler and
the search exploded. E7c keeps that and zeroes `ToblerSlopeContributor` on
**mesh edges only** (graph edges keep it — there the contributor *is* the
slope model, with no solver-side term).

| lane | corpus hash | vs baseline | DEM lookups | vs baseline | solved |
|---|---|---|---|---|---|
| off-trail | `2cdb4466bf632d6b` | **changed** | 1 692 436 | **−11.9%** | 12/12 |
| unified | `4417da3360f1ce8b` | **changed** | 189 715 | **−24.1%** | 12/12 |

### What it establishes

Together, E7b and E7c pin down A12 exactly:

| Term | Remove it → | Verdict |
|---|---|---|
| solver's `tobler_pace(grad)` — **directional** | search explodes (>5000× slowdown) | **structural** |
| contributor's `ToblerSlopeContributor` — **isotropic, east–west probe** | routes change, solver fine, 12–24% less DEM work | **additive, not structural** |

So the double-count is **real and removable**: both terms charge for slope,
but only one carries the direction the A\* needs. The contributor's term is
a genuine *extra* slope charge computed on an arbitrary axis.

Three consequences:

1. **The fix is known and small** — the E7c patch itself: return 0 from
   `ToblerSlopeContributor::contribute` for `EdgeKind::Mesh`.
2. **It is a calibration change**, not cleanup. Routes move, so it needs a
   baseline update and a full-corpus gate.
3. **It is also a performance win**: 12–24% fewer DEM samples, because the
   contributor samples N+1 points per edge. That is the single largest
   measured DEM-work reduction available.

---

## E3 + E5 re-confirmed on the unified lane

The outstanding caveat on both nulls was that they were measured only in the
default off-trail FMM lane. Re-run with **both** probes applied
simultaneously (legacy `SlopeLayer` multiplier ×37 + 11, **and** the native
contributor stack reversed):

| lane | corpus hash | DEM lookups | vs baseline |
|---|---|---|---|
| unified | `0575d4fd66c591a4` | 249 976 | **identical** |
| off-trail | `4558525db8df425c` | 1 921 389 | **identical** |

Both probes applied together move nothing, on either lane. An exact mutual
cancellation of a 37× cost perturbation against a stack reordering is not
credible, so this confirms each independently.

**E3 and E5 now hold on the lane production traffic actually uses.** The
caveat recorded against them is discharged.

---

## E10 — Pack slice fidelity **DONE. Halo answered — and a format defect found underneath it.**

**Method.** `tools/experiments/e10_packslice` slices the DEM (54% of the
pack) to each route's own bbox expanded by a varying halo, keeps mask and
graph whole, and asks at what halo the route reproduces whole-DEM geometry
bit-exactly. Payloads are copied **byte-for-byte** — no decode, no resample,
no recompression — so any difference is attributable to *missing* terrain,
never to *altered* terrain. That property is what makes the second finding
below provable.

### First attempt was confounded

The initial sweep sized every slice to the **union** of all five routes'
endpoints, handing each route a ~48 km effective halo. The sweep was
meaningless and non-monotonic. Re-run with per-route bboxes:

```
## lane: off-trail
halo_m      n-3496285  n-3894666  n-1821249   n-1884462  n-1895277  slice_MB  tiles
0               MATCH     DIFF+0      MATCH    DIFF-384      MATCH       0.9     10
500             MATCH     DIFF+0      MATCH       MATCH      MATCH       2.0     21
1000            MATCH     DIFF+0      MATCH       MATCH      MATCH       2.9     30
2000            MATCH     DIFF+0      MATCH       MATCH      MATCH       4.2     46
6000            MATCH     DIFF+0      MATCH       MATCH      MATCH      15.3    174
```
(unified lane identical in pattern; `n-1884462` differs by −24 pts at halo 0.)

### The halo answer

**500 m suffices** for every route that reproduces at all. Zero halo is
*not* enough — the longest route (7.1 km) loses 384 points without it.

Notably 500 m is far below the 3000 m `PAD_CAP_M` corridor bound, because
A\* prunes long before exploring the full padded rectangle. Slice cost at
500 m is roughly 2× the zero-halo size and still tiny (2.0 MB for five
routes).

**Recommendation: 1–2 km halo.** 500 m is the measured floor on five routes
in one region; the margin is cheap (a few MB) and the failure mode — a
silently different route near a pack edge — is expensive.

### The defect underneath: `Dem::sample` is not stable under slicing

`n-3894666` differs at **every** halo, including 6000 m. That cannot be
terrain starvation. `tools/experiments/e10_packslice --bin probe` samples
identical point grids from the full DEM and from each slice:

| halo | samples | **differ** | only_full | **only_slice** |
|---|---|---|---|---|
| 0 | 838 656 | 4 010 | 10 656 | 0 |
| 500 | 860 769 | 1 005 | 0 | **2 448** |
| 2000 | 914 370 | 1 286 | 0 | 3 008 |
| 6000 | 1 122 842 | 5 235 | 9 024 | 2 464 |

Observed disagreements are real elevation deltas: `975.525` vs `975.19995`,
`915.8` vs `914.39996` — up to ~1.4 m.

**The `only_slice` column is the airtight proof.** A slice is a strict
*subset* of the full DEM's tiles. Any point the slice can answer, the full
DEM can also answer. Yet at halo 500 there are **2 448 points where the full
DEM returns `None` and the slice returns a value.** The only mechanism that
explains that is tile-selection order.

### Mechanism

DEM v2 stores one entry per source raster with its own origin and builds an
rstar over tile bboxes at `open()`. Its own docs state: *"Tiles overlap only
when source rasters did (rare); a sample inside an overlap returns the
latest source's value (insertion order)."*

The Sjunkhatten DEM has **76 overlapping tile pairs** — the four DTM10 sheet
quadrants genuinely overlap, both horizontally (ulx 548435 vs 549795, a
1200 m band) and vertically (equal ulx, uly closer than the 2560 m span).
Inside an overlap two tiles both contain the point, and **which one answers
depends on rstar traversal order, which depends on the item set**. Slicing
changes the set, so the answer changes — and when the winning tile has
nodata there, `sample` returns `None` on the full DEM while the slice
succeeds.

**Not established:** which specific overlap band affects which route. The
proximity check performed tested route *endpoints* against band extents, not
the corridors actually sampled, and is too crude to attribute per-route.
`n-3894666`'s behaviour is *consistent* with this mechanism but not proven
to be caused by it.

### Consequences

1. **Pack parity is broken at the format level, independent of the halo.**
   "Same pack ⇒ same route" cannot hold while sampling depends on the tile
   set. This defeats the geometry-hash verification strategy for packs.
2. **The national artifact is itself build-order dependent.** Rebuilding
   with sources ingested in a different order yields different elevations in
   overlap bands — a reproducibility problem that exists today, with no
   packs involved.
3. Magnitude is small (0.12–0.62% of points, ≲1.4 m) but it **silently
   changes routes**, which is the worst combination.

### Fix — belongs in Phase F, before the slicer

Preferred: **de-overlap at build time.** Clip tiles to non-overlapping
extents when writing the artifact, so uniqueness is structural rather than a
runtime rule. Slightly shrinks the artifact and makes `sample` a pure
function of position.

Fallback: a **deterministic tie-break** in `Dem::sample` — when several
tiles contain a point, choose by a canonical rule (lowest `(ulx, uly)`
lexicographically, or an explicit priority field), never by rstar order.

Either way this is a **new prerequisite for the pack work** that the
implementation plan did not have, and it is cheap relative to discovering it
after packs ship.

---

## Summary — all twelve experiments

| | Question | Verdict |
|---|---|---|
| **E0** | Do transcendentals agree across ISAs? | Only `f32::atan` differs, ≤1 ULP |
| **E1** | Does the whole solver agree across ISAs? | **Bit-identical, both lanes** |
| **E2** | What does the elevation port cost? | **Below measurement resolution** |
| **E3** | Is the legacy cost channel dead? | **Yes** — but legacy is not inspect-only |
| **E4** | Is per-request rebuild viable? | **No** — 555 ms–2.8 s national |
| **E5** | Does contributor order matter? | **No**, both lanes |
| **E6** | What does `point_covered` claim? | Advisory layers grant coverage — **defect** |
| **E7** | Do the Tobler copies agree? | **Two different physical models**, 41.9% apart |
| **E7b** | Is the solver's slope term redundant? | **No — structural.** Removing it explodes the search |
| **E7c** | Is the contributor's slope term redundant? | **Additive, removable**, −12–24% DEM work |
| **E9** | Would config-as-string absorb change? | 3/31 commits, all additive |
| **E10** | Does a sliced pack reproduce routes? | **500 m halo suffices — but `Dem::sample` is unstable under slicing** |
| **E11** | Does the port API hold up? | Compiles and runs from memory |

Phase-0 caveat that applies throughout: **wall-clock numbers from single
runs are ±20%** on this host. Geometry hashes and DEM lookup counts are
exactly stable and carry every conclusion above.

---

---

## Phase E-1 — Landing E7c on the full corpus. **DONE. Kept.**

E7c measured the contributor's isotropic mesh slope term on a 12-hike
corpus and concluded it was additive rather than structural. This lands
it against all 90 Sjunkhatten hikes, and — unlike a refactor step —
judges it on **quality**, since a calibration change is supposed to move
geometry and the gate can only confirm that it did.

`tools/calibration_diff.py` compares two `eval-terrain` runs on mean
deviation from the walked truth, route length, and elevation gain.

### Off-trail lane

```
metric           before      after      delta   better   worse
dev_m              65.1       47.9      -17.2       59      27
length_m         1705.3     1675.5      -29.8       62      22
gain_m             60.8       60.5       -0.3       42      32

improved  59   total 1680 m   best single -466.1 m   median  -2.66 m
degraded  27   total  150 m   worst single  +96.6 m  median  +0.57 m
```

**Mean deviation -26%**, with 11 hikes improving by more than 20 m and
exactly one degrading by more than 20 m. DEM lookups 13 801 216 ->
11 901 773 (**-13.8%**); mean solve 251 -> 175 ms.

### Unified lane

```
metric           before      after      delta   better   worse
dev_m              14.8       13.6       -1.3       40      46
length_m         1624.5     1616.1       -8.4       65      20
gain_m             60.4       59.6       -0.8       44      31

improved  40   total  364 m   best single -223.0 m   median  -0.70 m
degraded  46   total  249 m   worst single +180.9 m  median  +0.64 m
```

DEM lookups 2 983 532 -> 2 441 330 (**-18.2%**).

### Why this was kept despite 46 > 40 on the unified lane

The raw win/loss count is the wrong summary here, and the tool's own
docs warn about the opposite failure — a change that rescues a handful
of routes while degrading many. This is the mirror image, and it earns
the same scrutiny rather than the same verdict:

- The **medians are sub-metre in both directions** (-0.70 vs +0.64 m).
  Most of the 86 routes that moved moved by less than the DEM's own
  10 m resolution, so counting them equally weights noise with signal.
- **Total displacement favours the change** in both lanes: 1680 m of
  improvement against 150 m of degradation off-trail, 364 against 249
  unified.
- The **tails are asymmetric in the right direction**: 11 routes
  improve by >20 m off-trail (3 unified) against one degrading in each.

### The one real regression

Hike **55810** (`sjunkhatten-long`, truth 3353 m) degrades in both
lanes — solver length 3526 -> 4093 m, deviation +96.6 m off-trail and
+180.9 m unified. A genuine loss, not a metric artefact: with the extra
slope charge gone, the router prefers a longer line it now prices as
cheaper. Recorded rather than explained away. Corpus mean *length* fell
in both lanes, so it is an outlier and not a trend.

### A note on the measuring instrument

`calibration_diff.py`'s first version computed distances on raw WGS84
degrees and reported `dev_m = 0.0` for every hike in both runs. That
reads exactly like "the change had no effect" — the most plausible
possible result, and completely wrong. Worth stating because it is the
same failure shape as E7b (a patch that measured nothing because the
code path never ran): a null result deserves a check that the
instrument can produce a non-null one.

### Baselines updated

```
full  off-trail 2b8239eaa049e85e  lookups=11901773  ok=89/90
      unified   5005c4cb2ccb9035  lookups=2441330   ok=90/90
ci    off-trail d27e60b7d508a594  lookups=2275940   ok=25/25
      unified   56f90de536583bd6  lookups=275229    ok=25/25
```

---

---

## Phase E-2 — Unify the pace curve. **NOT a calibration item. Blocked on a solver change.**

The plan listed "D3: unify pace curve" as Phase E calibration, on the
strength of E7: the mesh model is symmetric (a descent costs exactly
what the equal ascent costs) while the contributor's is real Tobler,
whose minimum sits at a 2.9 degree *descent* — a constant 41.9%
disagreement across all downhill.

Attempting it establishes that it cannot be done as a calibration
change, and the reason is structural rather than incidental.

### Why

`ToblerAnisotropic::metric_at` returns a **`SymMat2`** — a symmetric
quadratic form. The metric is **Riemannian**, so `G(v) == G(-v)`
identically, for every direction, at every cell. Uphill and downhill
along one axis are the same number because a symmetric matrix has no
way to hold two.

The pace function reinforces it: `tobler_pace` is handed `grad_mag =
sqrt(dz_dx^2 + dz_dy^2)`, a magnitude. The sign is discarded one line
before the call, so making `tobler_pace` signed would change nothing at
all — the information is already gone.

Asymmetric descent requires a **Finsler** metric (an asymmetric norm).
`tobler.rs`'s own header names that as future work. It is a solver
change with its own correctness burden — new discretisation, new
convergence argument — not a knob.

### What the state actually is after E-1

Worth stating plainly, because "two different Tobler models" now
describes something coherent rather than a drift:

| edge kind | model | why |
|---|---|---|
| mesh (off-trail) | symmetric | the metric is Riemannian and cannot be otherwise |
| graph (on-trail) | asymmetric, real Tobler | the direction of travel is known exactly |

E-1 removed the contributor's mesh-edge term, so the two models now
apply to **disjoint** edge kinds and no longer double-count. The
residual inconsistency is that an off-trail descent prices 41.9% higher
than the same descent on a trail, over and above the legitimate
off-trail penalty. That is an artefact of solver structure, and it is
the honest cost of a Riemannian metric.

### Pinned, not just asserted

`the_metric_cannot_distinguish_uphill_from_downhill` in
`tobler_aniso.rs` builds a real 30 degree ramp, takes the tensor through
`metric_at`, and checks that `v` and `-v` price identically for four
directions — plus a sanity assertion that the metric IS strongly
anisotropic, so the equality is not passing because everything is equal.
If someone later makes the tensor asymmetric, it fails and points at the
plan item that unblocks.

Writing it surfaced a second thing worth recording: `metric_at` returns
the **dual** metric `G*`, whose eigenvalues are reciprocal paces squared
— speeds, not costs. The first version of the sanity check asserted
along-fall-line > along-contour and failed, because a large value there
means *fast*. The doc comment says so; the test was written from the
name, not the documentation.

### Status

Deferred, with the reason recorded. The plan's Phase E list should read
one item, not two.

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
