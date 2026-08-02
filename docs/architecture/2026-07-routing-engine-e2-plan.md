# E2 — Proving the Port Abstraction Penalty

**Status:** in progress
**Bet under test:** #1 in the design rationale — *"port abstraction costs
<2% on the corpus DEM-work axis."* Every other part of the port design rests
on this; if it fails, the L1/L2 split has to be redesigned rather than
merely tuned.

---

## 1. What is actually being measured

The design replaces concrete artifact types with capability traits. Only
**one** of the four proposed ports sits on a hot path:

| Port | Calls per solve | On the hot path? |
|---|---|---|
| `Heightfield` (elevation) | **250 k – 1.9 M** | **yes** |
| `GeometrySet` (vectors) | ~10³–10⁴ (AABB-indexed) | no |
| `ClassField` (masks) | ~10³–10⁴ | no |
| `TraversalNetwork` | ~10⁵ edges, once per solve | no |

So **E2 = the elevation port only**. If elevation passes, the others are a
rounding error against it. This is a deliberate scope reduction, stated
explicitly so nobody later reads "E2 passed" as "all four ports are free".

### The surface is small

Every hot-path use of `Dem` in `turbo-tiles-pathfind` reduces to two
methods:

```
11 × dem.sample(...)
 5 × dem.slope_aspect(...)
```

(`clone` / `as_ref` are `Arc` mechanics, not trait methods.) Holders to
swap: **12 struct fields** across `contributor.rs`, `cost_field.rs`,
`fmm_adapter.rs`, `layers.rs`, `native_contributors.rs`.

```rust
pub trait ElevationLike: Send + Sync {
    fn sample(&self, p: PointXY) -> Result<Option<f32>, DemError>;
    fn slope_aspect(&self, p: PointXY) -> Result<Option<SlopeAspect>, DemError>;
    fn coverage(&self) -> DemCoverage;
}
```

Roughly 40 lines of trait plus a mechanical swap — a few hours, not a week.

---

## 2. A prediction, so the experiment can refute something

Running an experiment with no prior is how E7b produced a meaningless
number. So: a quantitative prediction first.

An indirect call through a vtable costs ~1–3 ns (indirect branch, normally
well predicted). Both memos (`EdgeElevProbe`, `CostField`) mean the *actual*
`dem.sample()` count equals the reported `dem_cache_lookups`:

| Lane | DEM lookups | Total solve | Predicted penalty @2 ns |
|---|---|---|---|
| off-trail (FMM) | 1 921 389 | 12 × 749 ms = 8 990 ms | 3.8 ms → **0.04%** |
| unified | 249 976 | 12 × 22.5 ms = 270 ms | 0.5 ms → **0.19%** |

**Predicted: 0.04–0.2%, well under the 2% budget.** If the measurement
comes back near that, the design's premise holds. If it comes back at 5%,
the cost is *not* dispatch — it is lost inlining cascading into the
surrounding arithmetic, which is a different problem with a different fix.

Stating this in advance is what makes the result meaningful either way.

---

## 3. Three arms

| Arm | Implementation | What it tells us |
|---|---|---|
| **A** baseline | concrete `Dem` (today) | reference |
| **B** dyn | `Arc<dyn ElevationLike>` everywhere | the **pessimistic bound** — what you get if nobody enforces the monomorphization rule |
| **C** generic | `CostField<H: Heightfield>`, `dyn` only at assembly | what the design **actually proposes** |

Arm B matters even though it is not the proposal. If **B** also lands under
2%, then the "generics in the hot loop, `dyn` at the edges" discipline is
*optional*, and the design simplifies — one fewer rule to enforce forever.
That is a genuinely useful outcome, so B is worth its build.

---

## 4. Both lanes, because E7b proved lane matters

`eval-terrain` defaults to `--mode=off-trail`, which dispatches to the FMM
solver; `--mode=unified` exercises the A\* that production traffic hits.
They stress the DEM completely differently:

| Lane | mean solve | DEM lookups | character |
|---|---|---|---|
| off-trail | 749 ms | 1.92 M | DEM-bound; dispatch amortised over heavy sampling |
| unified | 22.5 ms | 0.25 M | graph-bound; dispatch relatively *more* visible |

The unified lane is the harder test and the one that matters for users.
**Both arms run on both lanes.**

---

## 5. The gates that matter more than the timing

A speedup with changed output is a failure, not a win.

| Invariant | Why |
|---|---|
| `corpus_geometry_hash` identical across A/B/C | proves the swap is semantically transparent. Rust does no fast-math, so trait dispatch must not perturb a single float. |
| `dem_cache_lookups` identical across A/B/C | proves the memoisation structure is intact. A botched swap could double-sample and still be "fast enough". |

These are checked first. Timing is only interpreted if both hold.

---

## 6. The E7b guard — prove the probe executes

E7b's first run measured nothing because the patched code was never
reached. The guard here is structural rather than a runtime assertion:

> **Delete the concrete field type.** If `CostField` holds only
> `Arc<dyn ElevationLike>` (arm B) or `H: Heightfield` (arm C) and the
> crate compiles, the concrete path is gone by construction.

A type-level proof beats an assertion, and it cannot be misread the way the
`tobler_pace is never used` warning was.

---

## 7. Phase 0 — measure the ruler before the thing

**Running now.** Five identical runs per lane on unmodified code.

The 2% budget is only claimable if run-to-run spread is well below it.
Prior evidence is encouraging but thin (n=2: 749.3 vs 749.7 ms, 0.05%
apart). Phase 0 makes it n=5 per lane and reports mean, sd, and range.

- **spread < 1%** → the corpus harness can resolve 2%; proceed to arms B/C.
- **spread ≥ 1%** → it cannot, and E2 needs a criterion microbenchmark
  (§8) as the primary instrument instead.

Reporting a 2% effect measured with 3% noise would be worse than not
measuring at all.

---

## 8. Fallback instrument, if phase 0 says the corpus is too noisy

Criterion microbenchmarks isolating the two hot functions, driven at
realistic call volumes:

- `EdgeElevProbe::elevations(n)` — the per-edge sample memo
- `CostField::ensure(i, j)` — the per-cell contributor evaluation

Much lower variance, but **less ecologically valid**: it does not capture
the cache behaviour of a real solve. So the corpus stays primary where it
can resolve the effect, and the microbench serves as tiebreaker and
explanation ("the cost is dispatch" vs "the cost is lost inlining").

---

## 9. Effort

| Step | Cost |
|---|---|
| Phase 0 noise floor | running — no code |
| `trait ElevationLike` + impl for `Dem` | ~40 lines |
| Swap 12 holders, two cargo features for arms B/C | 1–2 h |
| Runs (3 arms × 2 lanes × 5 reps, ~6 min build each) | mostly machine time |
| **Total** | **~half a day plus machine time** |

---

## 10. What each outcome means

| Result | Consequence |
|---|---|
| **C < 2%, B < 2%** | Bet #1 holds *and* the monomorphization rule is optional. Design simplifies. |
| **C < 2%, B ≥ 2%** | Bet #1 holds as designed. The "generics in the hot loop" rule becomes a **CI-enforced invariant**, not advice. |
| **C ≥ 2%** | Bet #1 fails. Fall back to the rationale's stated escape: enum-dispatch over the shipped source set, or monomorphise the engine over a concrete `Sources` type parameter. **The port contracts survive either way — only the dispatch strategy changes.** |
| **Hash or lookup count moves** | The swap is not semantically transparent. Stop and fix before reading any timing. |

The third row is the one worth emphasising: E2 failing does **not** invalidate
the port design. It changes how ports are dispatched, not whether the
engine depends on capabilities instead of artifacts.
