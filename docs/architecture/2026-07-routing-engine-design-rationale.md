# Routing Engine — Design Rationale

**Status:** proposal
**Companion to:** `2026-07-routing-engine-modularization.md` (the *why now*)
and `2026-07-routing-engine-module-design.md` (the *what*).
This document is the *on what basis*: the load-bearing decisions, how they
map onto established architectural theory, where the design deliberately
departs from orthodoxy, and what measurement would falsify each bet.

---

## 0. Summary judgement

The design is **deliberately unoriginal**. It is three well-trodden patterns
composed: **Hexagonal** (ports and adapters) for data access, **Microkernel**
(minimal core + registered plugins) for composition, and **Functional Core /
Imperative Shell** for the layer split. Novelty in architecture is usually a
smell; the goal here is to land on shapes that have known failure modes.

Two aspects are genuinely not covered by the canon, and those are where the
real risk sits:

1. **A driven port on a hot numeric loop.** Hexagonal literature assumes
   driven ports are coarse — repositories called O(1) per use case.
   `ElevationSource` sits on a path called 50–250 k times per route. No
   mainstream architecture text addresses this; the resolution is
   language-specific (monomorphization), not architectural.
2. **Shipping calibration constants inside the data artifact** so two
   runtimes agree bit-for-bit. That is closer to reproducible-build and
   immutable-artifact discipline than to any software-architecture pattern.

---

## 1. The load-bearing decisions

Ranked by what collapses if the decision is wrong.

### D1 — Invert the dependency on data access

*Cost contributors and solvers depend on capability traits, not on concrete
artifact types.*

Everything else rests on this. Offline packs, DEM-resolution swapping,
in-CI testing, and adding marsh data are all the same move: substitute an
implementation behind a port.

**Theory:** Dependency Inversion Principle; Hexagonal Architecture
(Cockburn, 2005) — `ports::data` are *driven* (secondary) ports, `L6` hosts
are *driving* (primary) ports. Textbook placement.

**Falsifiable by:** the abstraction penalty. Budget is <2% on the corpus
DEM-work axis. §4 covers what happens if that is blown.

### D2 — Cost is additive walk-seconds, not multipliers

Already made and validated in this codebase — `contributor.rs` documents
the calibration failure that multiplicative composition caused ("turn one
knob, break another scenario"). The design **inherits it unchanged** and
finishes the migration by deleting the legacy generation.

**Theory:** Hickey's *complecting* — the multiplicative model braided eight
sources at three lifecycles into one opaque scalar with no shared unit.
Also "make illegal states unrepresentable" (Minsky): the newtype proposal
(`WalkSeconds`) moves the unit rule from documentation into the type system.

**Note on where this is already strong:** the explicit separation of
`contribute()` (additive), `veto()` (refusal, with a reason label) and
`pace_factor()` (multiplicative, for effects that genuinely scale) is a
better factoring than most cost models achieve. It survives intact.

### D3 — The FFI boundary is a coarse façade, never a port

**Theory:** Fowler's *First Law of Distributed Object Design* — "don't
distribute your objects." The façade is PoEAA's **Remote Facade**, and
`Route` versus the internal `Candidate`/`Path` is PoEAA's **Data Transfer
Object**. The existing `route_plan.rs` already implements exactly this
("deliberately narrow and decoupled from the internal `Path` / debug
surface") — the design generalises a discipline the codebase already has.

**Falsifiable by:** nothing, really. This one is close to settled: a
port-level FFI costs 5–25 ms/route in marshalling and destroys the memo
locality that cut DEM work by 93%. If measurement contradicted that it
would be extraordinary.

### D4 — Composition is declarative; wiring is a registry

**Theory:** the **Microkernel** pattern (POSA vol. 1) — minimal core plus
plugins registered at boot; equivalently Martin's "plugin architecture."
The `EngineBuilder` is a **Composition Root** (Seemann): one place that
knows every concrete type, with no DI container.

**This is a real trade, not a free win.** Typed Rust builders would let the
compiler prove a wiring is valid. Registries replace that with *boot-time
validation*, which is strictly weaker. It is justified only because the
configuration must cross an FFI string boundary and a pack manifest — if it
didn't, typed builders would win.

**Falsifiable by:** the inner-platform effect (§3.5). If `EngineConfig`
acquires conditionals, we have built a bad programming language.

### D5 — Orchestration is separated from computation

*Solvers solve one leg on one corridor. They never split waypoints, retry,
cache, or choose strategy.*

**Theory:** Functional Core / Imperative Shell (Bernhardt) — L0–L2 are pure
computation over injected data; L3 is I/O; L4 sequences. Also **Sans-IO**
(the Python `hyper`/`h11` lineage): protocol/algorithm implementations that
never touch a socket or a filesystem, so they can be driven from tests.
Invariant #2 in the module design (no `std::fs`/`memmap2`/`tokio` below L3)
is the Sans-IO rule stated as a fitness function.

**Payoff:** `RouteStrategy` becomes an extension point. Round-trip,
loop-of-length-N, multi-day-with-huts and k-alternatives are all
orchestration over an *unchanged* solver and cost model — which is the test
of whether the seam is in the right place.

### D6 — The core is region-agnostic; the profile is an anti-corruption layer

**Theory:** DDD **Anti-Corruption Layer** — the profile maps N50/FKB
classes onto a neutral `Surface` enum so national vocabulary never reaches
the domain. Also DDD **Ubiquitous Language**, which this repo already
practises deliberately (`CONTEXT.md` is a glossary explicitly "devoid of
implementation detail").

**Falsifiable by:** if no second profile ever appears, the ACL is pure cost.
Mitigation: the profile also buys packaging (a crate that can be vendored
into a cdylib) and testability (`turbo-profile-test`), so it is justified
even at N=1 regions.

### D7 — Observability is a port

**Theory:** dependency inversion applied to telemetry — the standard
alternative (ambient global logger, thread-local recorder) is exactly what
`solver_trace.rs` does today, chosen to avoid signature churn. Once
`SolveContext` is threaded there is no churn to avoid, and a port buys the
`ndjson` sink, which is the only practical way to debug a device-only
divergence.

### D8 — The corpus is the arbiter

**Theory:** Feathers' **characterization tests** / golden-master testing.
This is not a new decision — `tools/ROUTING_DEV_LOOP.md` already implements
a server-free, deterministic, geometry-hash-gated loop with baseline
acceptance and a `--check-determinism` mode. It is the single reason a
restructure of this size is tractable at all.

**Theory, second reading:** Gall's Law — "a complex system that works is
invariably found to have evolved from a simple system that worked." A
big-bang restructure of a working router would be the classic violation.
The ten-step sequencing, each step independently revertable and
corpus-gated, is the answer to Gall: this is evolution under measurement,
not replacement.

---

## 2. Pattern inventory

| Pattern / principle | Where | Fit |
|---|---|---|
| Hexagonal (ports & adapters) | L1 ports, L3 adapters, L6 hosts | **Strong** — driving/driven split is textbook |
| Microkernel / plugin architecture | registries + profile | **Strong** |
| Functional Core / Imperative Shell | L0–L2 pure, L3 I/O, L4 sequence | **Strong** |
| Sans-IO | invariant #2 | **Strong**, and mechanically checkable |
| Composition Root (Seemann) | `EngineBuilder` | **Strong** — no DI container |
| Anti-Corruption Layer (DDD) | `turbo-profile-no` | **Strong** |
| Remote Facade + DTO (PoEAA) | FFI façade; `Route` vs `Candidate` | **Strong** — already practised at `route_plan.rs` |
| Strategy | `Solver`, `RouteStrategy` | **Strong** — replaces an `if` |
| Decorator / Composite | `PyramidElevation`, `CachedElevation` | **Strong** — composition *within* a layer |
| Interface Segregation | four data ports, not one `DataSource` | **Strong** |
| Stable Abstractions / Stable Dependencies | `core`: I≈0, A≈1; adapters: I≈1, A≈0 | **Strong** — both on the main sequence |
| Acyclic Dependencies | the downward-only layer rule | **Strong** |
| Liskov substitution | solver conformance suite | **Good** — behavioural subtyping is only checkable via a shared contract test, which is what this is |
| Open/Closed | registries | **Good** |
| Fitness functions (Ford/Parsons/Kua) | §12 invariants | **Good**, and consistent with existing practice (`test/architecture/*.dart`) |
| Characterization testing (Feathers) | the routing dev loop | **Strong**, already exists |
| Pipes and filters | planner stages | **Weak** — see §3.1 |
| Parnas information hiding | most modules | **Mixed** — see §3.1 |
| Deep modules (Ousterhout) | most modules | **Mixed** — see §3.2 |

---

## 3. Where it departs from orthodoxy — honestly

### 3.1 The planner is decomposed by flowchart, which Parnas argues against

Parnas (1972), *On the Criteria To Be Used in Decomposing Systems into
Modules*, makes precisely one argument: decompose by **design decisions
likely to change**, not by processing steps. His KWIC example shows the
flowchart decomposition losing badly.

`intake → feasibility → repair → overlay → legs → dispatch → stitch` **is a
flowchart decomposition.** That is the weakest module boundary in the whole
design and it should be named as such rather than defended.

The defensible version: each stage happens to also hide a *policy* that
changes independently — what counts as covered, how an endpoint is
repaired, how presets merge with overrides, how legs are cached and keyed.
Those policies are the real modules; the pipeline is just the order they
run in.

**Practical rule that follows:** if a stage does not hide a policy, inline
it. A stage that is only a sequence position is not a module.

### 3.2 Seven layers risks Ousterhout's "classitis"

*A Philosophy of Software Design* argues for **deep modules** — small
interface, large implementation — and against thin layers whose methods
pass through.

Testing each module against that standard:

| Module | Interface | Implementation | Verdict |
|---|---|---|---|
| `turbo-fmm` | 3 traits | eikonal + anisotropic + elastica numerics | **Deep** |
| `ElevationSource` | 4 methods | tiled zstd mmap + LRU + rstar | **Deep** |
| `Engine` | 6 methods | the whole system | **Deep** |
| `CostContributor` | 4 methods | 18 implementations | **Deep** |
| `turbo-geom` | ~8 functions | pure predicates | Thin, but it is a *kernel*, not a layer |
| `turbo-route-observe` | 1 trait | 4 sinks, 2 of them trivial | **Borderline** |

`turbo-route-observe` earns its place only because `ndjson` (device
capture) and `stream` (bounded decimation) are real work; `noop` and
`memory` are near-trivial. If it stays borderline it should be folded into
`core` as a module rather than a crate.

**The stronger defence of the layer count:** several "layers" are one crate
and exist to state a dependency rule, not to add indirection. No call
crosses all seven — a solve touches L4 → L2 → L1 → L3 → L0. Ousterhout's
objection is to *pass-through*, and there are none: every layer transforms.

### 3.3 `Projection` fails the two-implementation rule — and is kept anyway

"Speculative generality" (Fowler's smell catalogue) says an abstraction with
one implementation is a liability. There is one real CRS in this system,
and "which CRS" is arguably *not* a decision likely to change — failing
Parnas's criterion too.

It is kept on a **different justification**: information hiding of an
ambient global. `wgs84_to_utm33n` currently lives in `turbo-tiles-elev`,
which means an *elevation primitive owns the system's coordinate reference*
and 44 sites in `pathfind` depend on that placement. The port exists to
delete a global, not to enable variation. Fixture frames (`LocalTangent`)
give it a genuine second implementation, which is what keeps it honest.

Every other port passes the two-implementation rule with real
implementations: `ElevationSource` (artifacts, pack, memory, pyramid),
`FieldSource`, `FeatureSource`, `NetworkSource` (artifacts, pack, memory),
`Solver` (unified, FMM, Dijkstra), `Observer` (4 sinks).

### 3.4 `turbo-route-engine` sits in the zone of pain

Martin's I/A metrics: the engine is **concrete** (low abstractness) and
**widely depended upon** (low instability) — the quadrant that is hard to
change and hard to extend.

This is structural and cannot be designed away: orchestration is inherently
concrete and inherently central. Two mitigations:

- Hosts depend on it *thinly* — six methods, all of which serialize.
- `RouteStrategy` is the pressure valve. Orchestration accretes; if new
  behaviour lands as strategies rather than as `Engine` methods, the engine
  stays small. **`Engine` is the module most likely to become the next god
  object**, and the design should be re-audited against that specifically.

### 3.5 Declarative config risks the inner-platform effect

If `EngineConfig` acquires conditionals, expressions, or references between
entries, we will have built a poor programming language inside TOML.

**Hard rule:** the config is a *finite record of choices* — which sources,
which contributors with which scalar parameters, which solver, which
budget. Never control flow. Anything needing a conditional is a
`RouteStrategy` or a contributor, written in Rust.

### 3.6 Second-system effect

Brooks' warning: the redesign after the first working system over-
generalises. The guards are the two-implementation rule (§3.3), the
"stage must hide a policy" rule (§3.1), and the fact that every port here
traces to a *concrete, already-requested* need — device packs, DEM
resolution, marsh data, algorithm swapping — rather than a hypothetical.

---

## 4. Where the theory runs out

### 4.1 A driven port on a hot loop

The canonical hexagonal example is a repository fetched once per use case.
`ElevationSource` is called 50–250 k times per route after memoisation.
Nothing in Cockburn, Martin, Evans, or POSA speaks to this: the OO pattern
literature was written where a virtual call is free relative to the work it
guards, and here it is not.

The resolution is language-specific and therefore *outside* the
architecture: `dyn` at assembly/registry level, generics and
monomorphization in the inner loop (`CostField<E: ElevationSource>`),
exactly as `turbo-fmm` already does with `solve_2d_with_metric<M: Metric>`.

**This is the design's largest technical bet.** If the <2% budget is blown,
the fallback is *not* to accept the cost and *not* to abandon the ports —
it is to narrow where dynamism is allowed: generate an enum-dispatch over
the shipped source set, or monomorphize the whole engine over a concrete
`Sources` type parameter with `dyn` retained only for the observer. The
port *contracts* survive either way; only the dispatch strategy changes.

### 4.2 Data and calibration as one versioned artifact

The pack carries its own `cost-config.toml` and presets so that a server
solve and a device solve of the same pack produce identical geometry. No
software-architecture pattern covers this. The closest analogues are
reproducible builds and immutable deployment artifacts — the pack is a
*build output*, versioned with `engine_min`, and an engine too old refuses
it rather than silently routing differently.

This turns "does the device agree with the server?" from a hope into an
assertion the existing geometry-hash harness can check directly.

---

## 5. The bets, and what would falsify them

| # | Bet | Falsified by | Fallback |
|---|---|---|---|
| 1 | Port abstraction costs <2% | corpus DEM-work axis regression | narrow dynamism (§4.1); contracts survive |
| 2 | Deleting the legacy cost model changes no geometry | corpus geometry-hash diff | it *is* the gate; a diff means the port was wrong, not the plan |
| 3 | Config-as-string means new sources need no ABI change | any new source requiring an FFI signature | the source needed a new *port*, not a new adapter — rare, and a real signal |
| 4 | Orchestration stays thin because strategies absorb growth | `Engine` gaining methods per feature | re-audit §3.4 |
| 5 | One profile is enough structure for N regions | a second region needing core changes | the ACL leaked; find what national vocabulary reached L1–L4 |
| 6 | Pack parity is bit-exact | device geometry hash ≠ server | a float-determinism issue, not an architecture issue — but it invalidates the offline story until fixed |
| 7 | Seven layers do not become pass-through | a module that only forwards | collapse it; layers are dependency rules, not obligations |

---

## 6. What this design is *not*

- **Not Clean Architecture proper.** Martin puts *use cases* at the centre,
  inside adapters and outside entities. Here the centre (L1) is contracts
  and the use case (`planner`) sits at L4, *outside* the services it
  orchestrates. This is Hexagonal + Layered, and the distinction matters
  when someone reaches for a Clean Architecture idiom that assumes use cases
  are innermost.
- **Not microservices.** All of it compiles into one binary or one cdylib.
  The layer boundaries are compile-time, not network. Applying distributed-
  systems reasoning here would be a category error — and D3 is precisely the
  argument against distributing across the hottest boundary.
- **Not a plugin system with dynamic loading.** Registration is
  compile-time so monomorphization stays available.
- **Not event-driven.** The observer emits; nothing subscribes to change
  state. Telemetry only.

---

## 7. The essential/accidental ledger

Brooks' distinction, applied honestly.

**Accidental complexity removed:** the dual cost-model generation (every
layer written twice); `Prefs` braiding four concerns; twelve
responsibilities in one type; per-request config resolved separately inside
two solvers; corridor sizing implemented twice; a Theta\*-shaped event
vocabulary describing solvers that no longer exist; an `include_str!`
reaching outside its crate.

**Accidental complexity added:** seven layers where there were three;
registries where there was direct construction; a config schema where there
were function arguments; `dyn` where there was a concrete type.

**Net:** positive only if the variation points are real. They are the
user-stated requirements — swap algorithms, swap DEM sources and
resolution, add data sources cheaply, decouple from Norwegian data, package
for offline, debug visually. Every added abstraction traces to one of
those. If any of those requirements were speculative, the corresponding
abstraction would be net-negative, and §3.3's rule is what catches it.
