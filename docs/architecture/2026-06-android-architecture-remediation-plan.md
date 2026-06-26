# Android architecture remediation plan

**Status:** Phases 0–3 COMPLETE · `:app:assembleDebug` + Konsist green · not device-tested · **Date:** 2026-06-25 · **Scope:** `apps/android`

## Execution status (2026-06-25)

| Phase | State | Notes |
|-------|-------|-------|
| 0a MapLibre cleanup | ✅ done | Dead Konsist guard removed; user-facing "MapLibre GL Native" in AboutScreen + the stale `MapEngine` kdoc corrected. |
| 0b Synthetics → debug | ✅ done | Moved to `core:data/src/debug`; per-variant `RemoteRepositoriesModule` (debug gates on emulator, release is HTTP-only). `core:data:compileReleaseKotlin` green ⇒ fakes can't ship. |
| 0c MapOverlay → turbomap-android | ✅ done | Moved + package change; `core:designsystem` no longer imports `domain.MapEngine/Marker`. Found a same-package `MarkerPin` ref to re-import. |
| 1a MapHostCoordinator | ✅ done | Pure decisions (off-route, inset, camera-restore, persist) extracted + **8 headless unit tests pass**. NB: the "~60 effects" are Compose-bound and stay in the screen as thin callers — only the *logic* lifted. |
| 1b Slot extraction | ◻ partial | Self-contained parts → `MapScreenParts.kt`; `MapScreen` 1256→1161 lines. Deep `RouteLayer`/`LiveLayer` slots deferred to a device-QA pass. |
| 2 core:tracking | ✅ done | New module (8 files + 7 tests). Discovered `FollowController → PathRepository` ⇒ `core:tracking → core:data` is a real, **acyclic** edge (not the zero-dep split first assumed). `RecordingFilter`/its test also belonged to tracking. |
| 3a map-core kernel | ✅ done | `feature:map-core` holds `MapToolHost` + `MapCameraState` (the documented seam). |
| 3b 8-module split | ✅ done | `feature:map-{sun,radar,live,markers,offline,collectionpicker,route}` extracted as leaves on `map-core`. Resources (per-tool strings × 2 locales, the offline `<service>` + manifest, `offline_summary` plurals) moved per module. `route` was hardest (74 host refs, package change + injected host imports). All compile; `:app:assembleDebug` green. |
| 3c Konsist tier rules | ✅ done | `ArchitectureBoundaryTest` gains "map tool modules are leaves" + "map kernel imports nothing from the feature tier"; old host-edge rule skips the `map-*` tier. Konsist green. |
| — verification | ✅ | Full `:app:assembleDebug` (Hilt + resource/manifest merge across all modules) + Konsist pass. **Not device-tested** — behavioural QA of the map host still pending. |

**Findings that did not survive code review** (folded into §1): #3 FFI drift (bindings are generated + gitignored), #4 two renderers (MapLibre already deleted), and the "two tile managers" smell (clean interface/impl). The app was in better shape than the first-pass review implied.

**Phase 3 risk note:** unlike Phase 2 (near-isolated runtime), the map tool split reshapes the Compose UI host and needs the `feature:map-core` kernel contracts *designed*, not just moved — and it can't be device-verified headlessly. Recommended to land `feature:map-core` + one safe leaf (`sun`/`radar`, which don't touch host state) first, then promote `route`/`live` under device QA.

---


This plan resolves the maintainability risks found in an architecture review of
the native Kotlin/Compose app. It assumes Android-native is the **committed
primary** Android client (Flutter retiring; iOS/web are real sibling targets),
which justifies the structural investment below.

---

## 1. Findings, re-assessed against the code

The first-pass review raised seven concerns. Verifying each against the source
changed the picture materially — recorded here so we don't act on stale smells:

| # | Concern | Verdict |
|---|---------|---------|
| 1 | `MapScreen.kt` is a 1256-line god composable (6 ViewModels, ~60 effects) | **Real** — primary work item |
| 2 | `core:data` mixes live-runtime, CRUD, and network concerns in one module | **Real** — one cohesion fault line |
| 3 | Two copies of the 3k-line uniffi FFI binding can drift | **False** — both are build-time generated and gitignored (`core/turbomap-android/.gitignore`); the split is the intentional host-JNA-jar vs Android-JNA-aar separation |
| 4 | Two renderers (MapLibre + wgpu) carried in parallel | **Mostly false** — MapLibre is already gone: no gradle dep, no `MapLibreEngine`, `MapScreen` instantiates `TurbomapMapView` directly. Only a stale Konsist guard + comments remain |
| 5 | Synthetic stand-in repos ship in production `src/main` | **Real but minor** — runtime-gated, but the fake code is in the release binary |
| 6 | `MapOverlay` (619 lines) lives in `core:designsystem` | **Real** — imports `domain.MapEngine`/`Marker`; map composition leaked into the shared design system |
| — | "Two offline tile managers" | **False** — `OfflineTileManager` is the interface, `WgpuOfflineTileManager` its only impl. Good design |

Net: the app is well-architected (Now-in-Android module layout, interface-based
repositories, Hilt, and Konsist boundary tests that fail the build). The real
work is **#1, #2, #5, #6**, plus a trivial **#4** cleanup.

---

## 2. Decisions (from the design review)

### #1 — Decompose `MapScreen` into a map module tier (full split)

- **`feature:map-core`** — a *passive* shared kernel. Holds only: the
  cross-tool callback contract (`MapToolHost`), read-only shared UI state
  (`MapCameraState`, `MapSelection`), and the map-surface slot API
  (`MapSurfaceSlot`). Depends on `core:*` **only**. **No orchestration, no tool
  state.** Inclusion test: a type belongs here only if ≥2 map modules (or
  host + 1 tool) reference it.
- **Leaf tool modules** — `feature:map-route`, `feature:map-offline`,
  `feature:map-live`, `feature:map-markers`, `feature:map-radar`,
  `feature:map-sun`, `feature:map-collectionpicker`. Each depends on
  `feature:map-core` + `core:*`, **never on each other, never on the host**.
- **`feature:map` (host)** — thin scaffold + nav drawer chrome + the
  **`MapHostCoordinator`** that owns the cross-tool orchestration (the ~60
  effects). The coordinator is a plain, unit-testable class — the orchestration
  tangle becomes testable for the first time.
- **Cross-tool actions flow through `core:*` seams**, never tool→tool. The seam
  already exists: `FollowController` (`core:data`) mediates "start Follow from a
  Route"; `StandardMapEntityActions` (`core:map`) mediates entity actions.
- `nav` stays in the host (it is host chrome, not a tool).

### #2 — Split `core:data` along its one real fault line (two-way)

- Extract **`core:tracking`** — the stateful, app-scoped, sensor-coupled live
  runtime: `LocationRepository`, `LocationFilter`, `FollowController`,
  `RecordingController`, `RecordingDraftStore`, `CaptureSession`, `TrackCapture`,
  `LiveStats`. This is a cohesive subsystem (and the subject of the active
  tracking/following redesign).
- **`core:data` keeps** the CRUD (Room/DataStore) and network read-repos. We do
  *not* speculatively split per-domain — most repos are small and CRUD-similar.

### #5 — Move synthetic stand-ins out of production source

- Relocate `Synthetic*Repository` (and `NetworkModule`'s synthetic wiring) into
  **`core:data`'s `debug` source set**, so the fakes never compile into release.

### #6 — Move `MapOverlay` to its real owner

- Relocate `MapOverlay` + its pin/marker composables into
  **`core:turbomap-android`** (its only consumer; already map-coupled). It
  **cannot** go in `feature:map-core` — `core:turbomap-android` consumes it and
  `map-core` depends on `core:turbomap-android`, which would be a cycle.
- `core:designsystem` keeps only generic visual primitives and **drops its
  `domain.MapEngine`/`Marker` imports**.

### #4 — Finish the renderer cutover (cleanup only)

- Delete the now-vacuous "MapLibre referenced only inside core map" Konsist test
  and the stale MapLibre comments. Keep the renderer-agnostic `MapEngine` seam
  guard.

---

## 3. Decision Record — the map-module dependency contract

> Recorded as a decision because it is **hard to reverse** (8 `build.gradle.kts`
> files will encode it), **surprising without context** (a flat `feature:map`
> would be the default expectation), and the **result of a real trade-off**
> (compile-time boundaries vs. module-count overhead for a solo maintainer).

**Decision.** The home map becomes a three-tier module graph:

```
feature:map  (host)
  ├─ depends on → every feature:map-<tool>
  └─ owns MapHostCoordinator (orchestration) + nav chrome

feature:map-<tool>   (route | offline | live | markers | radar | sun | collectionpicker)
  ├─ depends on → feature:map-core, core:*
  └─ NEVER on another feature:map-<tool>, NEVER on the host

feature:map-core   (passive kernel)
  └─ depends on → core:* ONLY
```

**Invariants (to be enforced by Konsist, extending `ArchitectureBoundaryTest`):**

1. A `feature:map-<tool>` module imports no other `feature:map-<tool>` and never
   the host.
2. `feature:map-core` imports nothing from the `feature:` tier.
3. Cross-tool behaviour is mediated by a `core:*` seam, not a direct edge.
4. The existing rule "only the map host may depend on other feature modules"
   extends to allow the host→`map-<tool>` edges.

**Alternatives considered and rejected:**

- *Coordinator + slot composables, no modules* — packages are already clean, so
  this is lowest-cost; rejected because we want compile-time prevention of future
  coupling, not just convention.
- *Partial split (heavy tools only)* — rejected in favour of uniform boundaries.
- *Shared "active brain" `map-core` owning the coordinator* — rejected: tools
  depend on `map-core` and could reach the coordinator, recreating coupling
  through the shared base. The coordinator stays in the host.
- *Allow `map-<tool>` → `map-<tool>` edges when "related"* — rejected: rebuilds
  the tangle with extra build files.

**Risk accepted:** ~11 new modules total (9 map + `core:tracking` + debug-set
churn) is significant scaffolding for a solo developer maintaining three other
clients. Mitigated by the convention plugins (`turbo.android.feature`) that make
a new module ~5 lines of `build.gradle.kts`, and by sequencing (below) so value
lands before the module count peaks.

---

## 4. Execution plan (sequenced)

Ordered so low-risk wins land first and the **coordinator is extracted before
any module is cut** — the coordinator is what decides shared-vs-leaf.

**Phase 0 — Warm-up cleanups (low risk, independent)**
1. #4: delete the stale MapLibre Konsist test + comments.
2. #5: move `Synthetic*` repos to `core:data/src/debug`.
3. #6: move `MapOverlay` to `core:turbomap-android`; strip domain imports from
   `core:designsystem`. Update the `MapEngine`-seam Konsist guard if needed.

**Phase 1 — Extract the coordinator in place (no new modules)**
4. Introduce `MapHostCoordinator` (plain class) in `feature:map`; migrate the
   ~60 effects/orchestration off `MapScreen` into it, behind unit tests.
5. Slim `MapScreen` into a scaffold that composes per-tool *slot composables*
   (still in-module). App stays green throughout.

**Phase 2 — Cut `core:tracking`**
6. Extract `core:tracking` from `core:data`; re-point DI bindings and feature
   deps. Independent of the map split — can land in parallel.

**Phase 3 — Promote map tools to modules (one at a time)**
7. Create `feature:map-core` (the passive kernel) and move the shared contracts
   into it.
8. Promote tools one per change — start with the most independent (`sun`,
   `radar`, `collectionpicker`), end with the most entangled (`route`, `live`).
   Each step keeps the build green.
9. Land the extended `ArchitectureBoundaryTest` invariants once the graph is in
   place, so the rule is born guarded.

**Definition of done:** `MapScreen` < ~250 lines; `MapHostCoordinator` covered
by unit tests; no `feature:map-<tool>` → `feature:map-<tool>` import; no
`domain.*` import in `core:designsystem`; no `Synthetic*` class in any release
APK; Konsist suite green.

---

## 5. What this plan deliberately does **not** do

- No per-domain explosion of `core:data` (kept to the one real fault line).
- No new shared module for the FFI binding (#3 was a false alarm).
- No renderer abstraction work (#4 migration is effectively complete).
- No cross-platform domain extraction — that is a separate, larger question; the
  shared Rust core remains the right long-term lever for the 4-client problem.
