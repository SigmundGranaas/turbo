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

## Environment notes

- `rustc 1.94.1`, x86_64-unknown-linux-gnu, single target installed.
- No `~/turbo-artifacts`; `TILESERVER_ARTIFACT_DIR` unset. Experiments
  needing real data (E2, E3, E4, E5, E6, E10) are blocked on the
  Sjunkhatten dataset build.
- Docker daemon started manually (`sudo dockerd`); `pgrouting/pgrouting`
  tag `16-3.4-3.6` does not exist — image selection pending.
- aarch64 target not installed, so E1 needs `rustup target add` plus the
  NDK toolchain.
