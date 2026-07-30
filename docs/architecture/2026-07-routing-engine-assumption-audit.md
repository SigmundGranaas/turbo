# Routing Engine — Assumption Audit and Validation Plan

> **Outcome note (added after implementation).** This document records
> what was *proposed* and what the experiments *measured*; both stand.
> One proposal did not survive contact: `rebind` / `ParamSet` was built
> and then removed, because nothing called it. E4's measurement — that
> rebuilding costs 555 ms–2.8 s per request — is real and correct, and it
> justifies the design *if* per-request tuning exists. It does not. The
> seam is cheap to rebuild when a caller appears; carrying ~250 lines of
> unused API until then was not.


**Status:** working document
**Companion to:** the modularization analysis, the module design (rev. 2),
and the design rationale.

This is an adversarial re-read of my own proposal after rev. 1 shipped with
the composition root in the wrong place. It has two parts: the other flaws
that the same failure mode produced, and a set of experiments that test
every load-bearing assumption **on the current codebase, before any
refactor is written**.

---

## 1. The failure mode, so we can hunt its siblings

The rev. 1 error was not "I forgot a rule". It was:

> **A principle applied halfway, never traced against a concrete scenario.**

Dependency inversion was applied to data *access* (correct) but not to data
*construction* (the engine still built its own adapters). Nothing caught it
because no scenario was ever walked end-to-end — the game-engine case would
have exposed it in five minutes.

So the audit hunts for three signatures:

- **(a) halfway inversions** — an abstraction introduced but bypassed on
  one path
- **(b) unfalsified assertions** — numeric or behavioural claims with no
  measurement and no cheap way to get one
- **(c) untraced scenarios** — a design element never followed through a
  concrete failure or workflow

Every finding below is tagged with which signature produced it.

---

## 2. Findings

### A1 — Cross-platform float determinism is assumed, untested, and probably false *(b)*

**Severity: critical. This is the one that can invalidate the offline plan.**

Bet #6 in the rationale claims "pack parity is bit-exact": the same pack on
server and device yields identical geometry, verified by geometry hash. I
asserted this with zero evidence.

The evidence available says it is unlikely. `exp()` appears on the routing
hot path in **ten** places:

```
fmm/elastica.rs:145        1.6667 * (-3.5 * (grad_mag.abs() + 0.05)).exp()
fmm/tobler.rs:99           …
fmm/tobler_aniso.rs:169    …
pathfind/unified.rs:96     …
pathfind/native_contributors.rs:187, 198, 829
pathfind/layers.rs:344, 345
pathfind/pathfinder.rs:1955   powf(1.5)   ← the projection
```

`exp()` and `powf()` are libm, and libm is **not** bit-identical across
architectures, toolchains, or versions. Add FMA contraction (aarch64
contracts `a*b+c` where x86-64 without `-mfma` does not) and the arithmetic
differs before you reach the solver.

Why this is worse than an ordinary risk: **geometry hashing is the only
verification instrument the whole plan relies on.** If hashes differ across
ISA, then every parity claim, the device corpus, and the "same pack ⇒ same
route" contract need a different shape — tolerance-based comparison, or a
vendored software libm, or accepting that device routes are *equivalent*
rather than *identical*.

This must be tested **first** (§3, E0/E1), before a line of refactor.

### A2 — My own fix for per-request tuning is wrong, twice *(a, c)*

Module design §3.3 proposed `Tuning` as "a resolved struct of scalars"
in L1 that the engine receives. Two problems:

1. **It is a god-struct.** Every contributor's knobs must be enumerated in
   an L1 type. Adding a marsh parameter changes a model-layer type and
   recompiles profiles and hosts — which is *exactly* the `Prefs` problem I
   criticised, reintroduced one layer down. Signature (a): I fixed the
   symptom (`Prefs` as an FFI type) without fixing the shape.

2. **The obvious alternative is refuted by the code.** "Just rebuild the
   `CostModel` per request" fails because contributors carry expensive
   immutable state. `TrailProximityContributor::new` bulk-loads **three
   R-trees** over the whole graph, and its own comment notes the polyline
   variant "would cost ~1 GB of boot RSS". That is not per-request work.

**Correct design:** split each contributor into an expensive immutable
index and cheap parameters.

```rust
pub struct TrailProximity { index: Arc<TrailIndex>, params: TrailParams }

pub trait CostContributor {
    /// Cheap: clones an Arc and swaps scalars. Per-request tuning.
    fn rebind(&self, p: &ParamSet) -> Arc<dyn CostContributor>;
    /// For the leg-cache key (see A6).
    fn fingerprint(&self) -> u64;
    …
}
```

`Tuning` disappears. Per-request tuning is a `rebind` over the stack, which
is Arc clones plus scalar writes. E4 measures whether that is actually
cheap.

### A3 — Coverage semantics are undefined, and the current behaviour is wrong *(c)*

`Pathfinder::point_covered` is:

```rust
self.layers.iter().any(|l| l.covers(x, y))
```

**Any** layer claiming coverage marks the point covered. A point with no
DEM but inside a landcover mask passes the feasibility pre-check — and the
pre-check exists precisely to prevent "the solver builds a uniform mesh and
returns a straight line, semantically a lie" (its own comment).

My design inherited this and made it worse: `Terrain { height, network,
extent }` cannot compute true coverage at all, because the geometry sets
live inside contributors the engine cannot enumerate. `Engine::extent()`
would confidently report the wrong thing.

**Fix:** layers declare their role.

```rust
pub enum Requirement { Required, Advisory }
```

Coverage = intersection of `Required` layers. Absence of an `Advisory`
layer means "no contribution", not "no data". And the engine **receives**
its extent from the composition layer (which knows everything it opened)
rather than deriving it from the two handles it happens to hold.

This also finally distinguishes the two failure modes that are currently
conflated: *outside the data* (refuse) versus *inside the data, in a hole*
(route with a coverage penalty — which `DemCoveragePenaltyContributor`
already implements for Norway's ~6 k absent alpine tiles).

### A4 — `AttrView` was the composition-root error again *(a)*

To keep `fkb_type` and DNT marking out of the engine, rev. 2 routed
semantic edge attributes through an opaque `AttrView`. But a contributor
then does `attrs.str("fkb_type")` **per edge, on the hot path**. It is
still Norway-aware — the key name is Norwegian — only now the coupling is
dynamic, unchecked by the compiler, and slower.

That is the same halfway inversion: I moved *where* the coupling lives
without removing it.

**Correct anti-corruption layer:** the adapter maps national codes to
neutral typed enums **once, at load or build time**.

```rust
pub struct EdgeSemantic { surface: Surface, waymarked: Waymarked, access: Access }
```

`AttrView` survives only for genuinely profile-specific contributors that
ship inside a profile crate — the rare path, not the default one.

### A5 — The Tobler formula exists in six copies, mixing f32 and f64 *(c)*

```
fmm/tobler.rs:99            f32
fmm/tobler_aniso.rs:169     f32
fmm/elastica.rs:145         f32
pathfind/unified.rs:96      f32
pathfind/native_contributors.rs:187, 198   f64
pathfind/native_contributors.rs:829        f64
```

Each site is justified in comments as "own copy — a physical formula, not
shared solver code". The consequence is that recalibrating hiking pace is a
six-site edit, and the unified router (f32) and the contributor stack (f64)
can disagree on the same slope by more than an ULP.

My design never addressed this — a direct miss, since "one cost model, one
place" is the principle the whole cost layer is built on. The pace curve
belongs in `turbo-route-cost::terrain` once, in one precision, with the
kernel taking it as a `Metric` implementation.

### A6 — The leg-cache fingerprint cannot be computed as specified *(b)*

Module design §6.2 says to key the cache on the resolved `Overlay`, calling
it a "provably complete" key. But `Overlay.extra: Vec<Arc<dyn CostContributor>>`
is not hashable — trait objects have no `Hash`. As written the design
cannot compute the key it claims. Needs `CostContributor::fingerprint() -> u64`
as a trait requirement (folded into A2's trait change above).

### A7 — `Budget` and parity contradict each other *(c)*

`Budget::handheld()` clamps `max_cells`, which **coarsens `cell_m`**. A
coarser grid is a different discretisation, therefore a different route.
So `Budget::handheld()` and bet #6's "same pack ⇒ same geometry" cannot
both hold. I wrote both, one page apart, without noticing.

**Fix:** parity is defined as *same pack + same budget + same cost model*.
Device/server comparison must pin the budget, and the pack should record
the budget its baseline hashes were generated under.

### A8 — Contributor order is part of the numerical contract *(b)*

`compose_edge_walk_seconds` sums contributions in list order. Floating-point
addition is not associative. If the stack is built from a config list, then
**the order of rows in a TOML file can change route geometry** at the ULP
level, and ULP differences flip ties.

Not necessarily a problem — but it means order must be pinned in the pack
and treated as part of the contract, not an incidental. E5 measures whether
it actually bites.

### A9 — I over-claimed "the corpus runs in CI" *(b)*

`turbo-geodata-memory` with a synthetic heightfield gives fast unit and
property tests. It does **not** give corpus coverage: the corpus asserts
real routes over real Norwegian terrain, and a synthetic fixture cannot
validate them. I conflated two different things.

Honest version — three tiers:

| Tier | Data | Runs | Validates |
|---|---|---|---|
| Unit / property | `memory` adapter, synthetic | every commit, CI | invariants, solver conformance, no regressions in logic |
| Corpus | one small **real** pack (~100 MB, fetched) | CI nightly / PR | route quality, geometry hashes |
| Full evaluation | national artifacts (8+ GB) | workstation, `routing_loop.py` | calibration, the four gating axes |

Getting the corpus into CI is a **data-distribution** problem, not an
architecture one. The architecture makes it *possible*; it does not make it
free.

### A10 — Kernel observability: concern withdrawn *(resolved)*

I was going to flag a contradiction between the new event vocabulary
(`CellSettled`, `FieldSnapshot`) and the rule that L0 does no observability.
It is not a contradiction — `solve_lifted_grade_limited` already takes
`Option<&mut dyn FnMut(&LiftedProgress)>`, so the precedent exists in the
kernel today.

One caveat that survives: the existing hook fires every 40 000 accepted
states. Per-cell `CellSettled` is three or four orders of magnitude more
events. That needs its own measurement (E8) before the vocabulary promises
it.

### A11 — `ClassField` may have no surviving implementations *(c)*

`routing_setup.rs` already skips raster landcover masks when a vector
collection of the same name exists (`taken_layer_names`), because the
vector layers are strictly better. If the vector migration completes,
`ClassField` has **zero** real implementations — speculative generality by
the rationale's own two-implementation rule.

Check before building it. It may fold into `GeometrySet` with a
raster-backed implementation, or survive for genuinely raster-native data
(snow depth, avalanche danger) that has no polygon form.

---

## 3. Validation plan

The point of this section: **almost every assumption can be tested on the
current codebase, before any refactor.** The instrument already exists —
`tools/routing_loop.py` gives a server-free, deterministic, geometry-hash
gated PASS/REGRESS verdict across four axes, and `eval-terrain` runs the
corpus in-process.

Each experiment states its question, its method, its cost, and **what
result kills or reshapes the plan**.

### E0 — Does `exp()` agree across architectures? *(A1)*

- **Method:** a 20-line program that evaluates
  `1.6667 * (-3.5 * (x + 0.05)).exp()` over 10⁶ inputs in both f32 and f64,
  hashes the outputs, and runs on x86-64 and aarch64.
- **Cost:** ~1 hour.
- **Kills:** if the hashes differ, bit-exact pack parity is dead in its
  current form and A1's mitigations must be designed before anything else.
- **Do this first.** It is the cheapest test with the largest blast radius.

### E1 — Does the corpus reproduce across architectures? *(A1, on-device viability)*

- **Method:** cross-compile `tileserver eval-terrain` for
  `aarch64-linux-android`, run `terrain-corpus.toml` against a small
  artifact set on a device or emulator, diff geometry hashes and latency
  against the x86 baseline in `routing-baseline.json`.
- **Cost:** 1–2 days, mostly cross-compile setup that the port needs anyway.
- **Kills:** hash divergence ⇒ redesign the parity contract. Latency >5×
  ⇒ the on-device story needs the budget work before anything else.
- **Bonus:** this is also the first real measurement of device solve time,
  which I estimated at 1.5–3× with no evidence.

### E2 — What does dynamic dispatch on the elevation path actually cost? *(bet #1)*

- **Method:** on the current tree, no refactor. Introduce a one-method
  `trait ElevationLike` implemented by `Dem`, and change `EdgeElevProbe`
  and `DemElevation` to hold `&dyn ElevationLike`. Run `routing_loop.py`.
  Then repeat with a monomorphized generic to bound the other end.
- **Cost:** ~half a day, ~50-line diff.
- **Kills:** >2% on the DEM-work or latency axis ⇒ narrow where dynamism is
  allowed (enum dispatch over the shipped source set, or monomorphize the
  engine over a concrete `Sources` type). The port *contracts* survive
  either way; only dispatch changes.

### E3 — Is the legacy cost stack really geometry-neutral? *(bet #2)*

- **Method:** on the current tree, bypass the legacy `CostLayer`
  contributions in the solve path (they are supposed to serve only the
  inspect endpoint and the build-time refusal sampler). Run the corpus.
  Geometry hashes must be **identical today**.
- **Cost:** ~1 hour, ~10-line diff.
- **Kills:** any hash change means the legacy stack is still load-bearing
  somewhere, and step 2 of the sequencing (1.5 weeks) is not the mechanical
  deletion it is planned as.
- **Best ROI in the list:** one hour to de-risk a week and a half.

### E4 — How expensive is contributor construction? *(A2)*

- **Method:** criterion bench on `Pathfinder::with_defaults_and_config`,
  and separately on `TrailProximityContributor::new` (already known to
  bulk-load three R-trees).
- **Cost:** ~1 hour.
- **Determines:** the split point between `Arc<Index>` and `Params`, and
  whether `rebind` per request is viable at all.

### E5 — Does contributor order change geometry? *(A8)*

- **Method:** permute the `push_with_native` order in `routing_setup.rs`,
  run the corpus, diff geometry hashes.
- **Cost:** ~1 hour.
- **Determines:** whether stack order must be pinned in the pack contract.

### E6 — What does `point_covered` actually claim? *(A3)*

- **Method:** query points inside a known DEM hole (Jotunheimen /
  Sognefjell, per the `DemCoveragePenaltyContributor` comment) that are
  covered by a landcover mask. Check `point_covered` and then solve
  through them.
- **Cost:** ~1 hour.
- **Determines:** confirms the `.any()` bug and fixes the Required /
  Advisory semantics against real data rather than in the abstract.

### E7 — Do the six Tobler copies agree? *(A5)*

- **Method:** evaluate all six implementations over a shared slope range;
  diff. Then check whether unifying them changes corpus geometry.
- **Cost:** ~2 hours.
- **Determines:** whether unification is free (pure cleanup) or a
  calibration change that needs a baseline update.

### E8 — What does per-cell event emission cost? *(A10)*

- **Method:** extend the existing `LiftedProgress` hook to a per-cell
  callback, run the corpus with a no-op closure, measure.
- **Cost:** ~2 hours.
- **Determines:** whether `CellSettled` / `FrontierState` stay in the event
  vocabulary or are replaced by post-hoc `FieldSnapshot` only.

### E9 — Would config-as-string really have absorbed past changes? *(bet #3)*

- **Method:** git archaeology. Take the last ~20 routing-affecting commits
  and ask of each: under the proposed design, would this have been a config
  edit, a contributor addition, or an ABI change?
- **Cost:** ~1 hour, no code.
- **Determines:** whether the central FFI-stability claim is supported by
  the project's actual change history rather than by argument.

### E10 — Does a sliced region reproduce national routes? *(pack fidelity)*

- **Method:** throwaway script slicing a DEM bbox plus a graph subset with
  a halo; run the corpus subset that fits inside; diff geometry against the
  national run.
- **Cost:** ~1 day.
- **Determines:** the halo/boundary policy, before the real pack pipeline
  is built.

### E11 — The portability conformance test *(the game-engine claim)*

- **Method:** write module design §3.4 as a real test now. It will not
  compile against today's code (`Dem` requires a file). It becomes the
  acceptance gate for the ports step.
- **Cost:** ~2 hours.
- **Determines:** nothing today — it is a *specification*, and the point is
  that "portable to a game engine" stops being a claim and becomes a
  build-breaking assertion.

---

## 4. A one-week de-risking sprint

Nothing here requires committing to the refactor.

| Day | Experiments | Answers |
|---|---|---|
| 1 (am) | **E0** | Is bit-exact cross-ISA parity even possible? |
| 1 (pm) | **E3**, E5 | Is legacy deletion free? Does stack order matter? |
| 2 | **E2**, E4 | What do ports cost? Where does the index/params split go? |
| 3–4 | **E1** | Does the corpus reproduce on-device, and how fast? |
| 5 | E6, E7, E8, E9, E11 | Coverage semantics, Tobler unification, event cost, ABI history, the conformance spec |

Outcome: every load-bearing number in the plan is measured rather than
asserted, and the three findings most likely to reshape the design (A1
parity, A2 tuning, A3 coverage) are settled against real data.

If E0 and E1 come back clean and E2 lands under 2%, the sequencing in the
modularization doc stands as written. If E0 fails, the offline verification
strategy is redesigned before anything is built — which is exactly the
outcome worth a week.

---

## 5. What this audit says about the process

Three of the eleven findings (A2, A4, and the rev. 1 composition root) are
the **same error**: a principle applied to one path and not to its sibling.
Two more (A1, A7) are **assertions I wrote adjacent to their own
contradiction** without noticing.

The generalisable guard is not more review. It is that **every structural
claim gets a cheap executable check attached at the moment it is written**:

- "portable to a game engine" → E11, a compiling test
- "no I/O below L5" → invariant #3, a grep
- "bit-exact parity" → E0/E1, a hash diff
- "legacy is inspect-only" → E3, a corpus run
- "ports are ~free" → E2, the DEM-work axis

A claim with no attached check is a claim that will be wrong for a while
before anyone notices — which is precisely what happened to rev. 1.
