# Web map — feature-slice modularization plan

**Status:** Designed (grilled) · not started · **Date:** 2026-06-26 · **Scope:** `apps/web/src` map layer

Resolves the two maintainability risks from the web-map architecture review: the
**794-line `MapScreen.tsx` god-component** and **tool↔tool coupling** (every panel
opener manually closes the others; `mapRef` prop-drilled everywhere). It mirrors the
committed cross-client model in `CONTEXT.md` ("Map Tool" = independent capability;
cross-tool interaction goes through a shared seam, never tool→tool) and the executed
Android split (`docs/architecture/2026-06-android-architecture-remediation-plan.md`):
a **kernel / leaf-features / host+coordinator** topology.

This is a **design doc**. Implementation is the strangler in §6.

---

## 1. Decisions (grilled)

| # | Decision | Rationale / alternative rejected |
|---|----------|----------------------------------|
| D1 | **Mirror the Android three-tier topology** (kernel / leaf-features / host+coordinator) on web | One mental model across clients; reuse `MapToolHost`/kernel/leaf vocabulary. Rejected: a web-native bespoke structure (divergence costs more than it saves). |
| D2 | **Modules = directory slices + public `index.ts` barrels + ESLint boundaries** | Standard React feature-slice idiom; enforced barriers without a build graph. Rejected: pnpm workspace packages (heavy, slow iteration for one consumer); convention-only (no enforcement). |
| D3 | **Two shared layers: `map-engine` (substrate impl) + `map-core` (passive contract kernel)** | Different change-rates/audiences (renderer/WASM vs cross-cutting seams). A feature needing only the overlay hook shouldn't transitively pull the WASM lifecycle. |
| D4 | **Engine access via `useMapEngine()` context returning a narrow `MapEngine` *interface*** (not the raw `TurboMap`) | Kills `mapRef` prop-drilling; features never name the WASM type; **stubbable in unit tests**; doubles as engine documentation. |
| D5 | **Engine *interface* lives in `map-core`; `map-engine` implements it** (dependency inversion) | Features depend only on `map-core`, never the engine impl. Keeps the kernel the single home of contracts. |
| D6 | **9 feature slices**, each owns its UI + data (api+hooks) + state (store) | Matches current dir grain; "slice owns UI+data+state" is the clean-arch cut that makes each feature independently understandable + testable. |
| D7 | **Cross-cutting placement rule:** used by ≥2 features ⇒ sink to a lower tier (features import *downward*) **or** rise to the **host** (if orchestration). Features never import each other. | The discipline `CONTEXT.md` already mandates. Applications: **sharing → `shared`**; **user-location → `map-core` kernel primitive**; **context-menu → host**. |
| D8 | **Panel orchestration = a pure visibility mutex.** The host/orchestrator owns one value `activePanel: PanelId \| null`; opening one hides the previous. Closing means *hidden*, never *reset*. | Collapses 5 scattered open-flags (an illegal "two panels open" state) into one value — exclusivity is intrinsic, no precedence cascade, no manual "close-the-others". Mirrors #188 "one selection model + detail host". |
| D9 | **State + lifecycle belong to the slice, in its store** (the panel is a *view* over the store). Persist-by-default while hidden. | The orchestrator holds no feature state, so it can't destroy any. Hiding unmounts the *view*; the *store* survives → reopen resumes. Routing keeps building behind a peeked marker. **Editor drafts stay component-local (lost on hide) for now** — a feature may promote its draft to its store later without touching the orchestrator. |
| D10 | **Host-side `switch` renders the active panel — no runtime registry.** Kernel holds only the `PanelId` string + `open/close/active`. | The host is the composition root (allowed to import every feature barrel); a `switch` is simpler, **type-exhaustive**, and still goes through public barrels. A registry buys no boundary benefit here and loses exhaustiveness. Same for overlays (host composes) and tap-dispatch. |
| D11 | **Host owns tap-dispatch**, calling feature APIs | The raw input stream is a shared resource like the panel slot — arbitrating it is orchestration. Not a behavior leak: the host decides *who gets the tap* and calls a feature's public `addWaypoint`/`openDetail`; it never implements the action. Generalize to a capturing-tool input chain when the 2nd capturing tool (measure/Live) lands. |
| D12 | **Enforce with `eslint-plugin-boundaries` (`element-types` + `entry-point`), gated in CI** | The web analogue of Android's Konsist `ArchitectureBoundaryTest`. `pnpm lint` added to `web_build` so a boundary violation fails the build (today CI runs only `tsc -b && vite build`). |

---

## 2. Target architecture — tiers & dependency rules

```
app  →  host  →  features/*  →  map-core  →  shared + ui
                      │             ↑
                 map-engine ────────┘   (implements map-core's MapEngine interface)
```

| Tier | May import | Owns |
|------|-----------|------|
| `shared` + `ui` | external only (`ui`→`shared` ok) | `apiFetch`, `geo` (+`haversineMeters`), `config`, `shareResource`; the design kit (Glass/Panel/Icon/Cookie/…) |
| `map-core` (kernel/contracts) | `shared`, `ui` | `MapEngine` interface · `useMapEngine()` · `MapToolHost` interface (`usePanels`/`useToast`) · `useProjectedLayer` · `useUserLocation` + `<UserLocationLayer>` · geo/camera types |
| `map-engine` (substrate impl) | `map-core`, `shared` | `TurboMap` lifecycle · `<MapSurface>` + rAF loop · `tileFetcher` · `scene`/`templates` · `gestures` · **provides** the `MapEngine` context |
| `features/*` | **`map-core`**, `shared`, `ui` | each slice's panels/overlays/api/hooks/store + typed `openX()` |
| `host` | features, `map-core`, `map-engine`, `shared`, `ui` | implements `MapToolHost`; mounts `<MapSurface>`; renders panel `switch` + overlays + context menu + chrome; `MapHostCoordinator` (orchestration); routes taps |
| `app` | host + providers | entry |

**Hard rules:** features never import each other / the host / `map-engine`; the kernel never imports features or the host; `shared`/`ui` import nothing internal.

### The kernel contract (`map-core`)

```ts
interface MapEngine {                 // map-engine implements; useMapEngine() returns it (null until boot)
  project(lat, lng): [number, number] | null;
  unproject(x, y): [number, number] | null;
  ease_to(...); set_camera(...); orbit_around(...); zoom_around(...);
  camera_json(): string; /* insets, sun, … */
}
interface MapToolHost {               // host implements; injected via context
  panels: { open(id: PanelId): void; close(): void; active: PanelId | null };
  toast(message: string): void;
}
```

A feature's public `openMarkerDetail(id)` = `markersStore.select(id)` (its own state) **and**
`panels.open('marker-detail')` (ask the host to make it the visible panel). Map-tap selection
highlight is *derived* from `panels.active` + the feature store, not a separate selection flag.

### Feature shape

A feature contributes **(a) zero-or-more always-mounted overlay/controller components** (mounted
at the map root by the host; gated on the feature's *store*, so they survive panel hiding) and
**(b) zero-or-one slotted panel** (a view over the store; mount/unmount = its enter/exit lifecycle).
Routing = SSE-controller + route-line (always-on) + planner (slotted). Markers = pins (always-on)
+ detail/editor (slotted). The **collection picker is a modal**, not the exclusive panel slot.

### The 9 slices

`markers · tracks · routing · conditions · collections · sun · basemap · search · account`
(each `src/features/<name>/index.ts`). Cross-cutting: `sharing`→`shared`; user-location→`map-core`;
context-menu→host.

---

## 3. `index.ts` (the module API)

A barrel exports only: the feature's **panel component(s)** + **overlay component(s)** (for the host
to compose), its typed **`openX()` actions**, its **query hooks** if another tier legitimately needs
them, and **public types**. Everything else (the store internals, API client, sub-components) stays
private — `entry-point` lint forbids deep imports.

---

## 4. Enforcement

`eslint-plugin-boundaries`:
- `boundaries/element-types` — encodes the §2 matrix (allowed import edges per tier).
- `boundaries/entry-point` — cross-element imports must resolve to the element's `index.ts`.

Wire `pnpm lint` into `.github/workflows/web_build.yml` as a gate (fails the image build on a
violation). Flip rules to `error` only after §6 completes, so the gate never blocks a half-migrated step.

---

## 5. What collapses

- `selectionStore`, `conditionsStore`, `uiStore.accountOpen`, and `pathsStore`'s open/tab/
  editingId/selected* → mostly **gone**: "is my panel open + which item" becomes the active-panel
  value + the feature's own minimal selection. Only **routing** keeps a substantial store
  (waypoints/preset/plan/SSE status — genuine in-progress work).
- The derived precedence cascade + manual "close-the-others" → **deleted** (one mutex).
- Triplicated rAF-`project()` overlay loops + redefined `DPR()` → one `useProjectedLayer`.
- Per-panel effects hoisted into `MapScreen` (frame-on-select, editor reset) → **into the feature panels**, driven by mount/unmount.

---

## 6. Migration — strangler (each step builds green + headed-smoke-verified; never big-bang)

1. **Substrate + kernel foundation** — extract `TurboMapCanvas`/`tileFetcher`/`scene`/`templates`/`gestures` → `map-engine`; `MapEngine` interface + `useMapEngine()` + `<MapEngineProvider>` in `map-core`. Overlays keep `mapRef` for now.
2. **Shared overlay hook** — `useProjectedLayer` → `map-core`; migrate `MarkerPins`/`UserLocation`/`RouteOverlay`; move `UserLocation` into the kernel; kill duplicated `DPR()`.
3. **Prove the slice shape** — `sun` first (engine-only, no panel), then `account` (trivial `panels.open()` seam).
4. **Panel features, one at a time** — `conditions` → `collections` → `markers` → `tracks`. Each: carve slice, introduce `openX()` + `panels.open(id)`, move panel behind host `switch`, delete old store/precedence code.
5. **Routing last** — two-part (mounted controller + slotted panel), persist-while-hidden, tap-capture — under headed QA.
6. **Collapse the host** — extract `MapHostCoordinator` (panel mutex, tap-dispatch, inset, toast, redeem-boot, frame-on-select) as a plain testable unit; `MapScreen` → thin scaffold; context-menu → host. Flip boundary rules to `error` + CI gate on. Add unit tests to the now-pure pieces (`trackImport`, coordinator decisions, gesture classification).

---

## 7. Risks

- **No TS test safety net** today → every strangler step must be headed-Playwright-smoke-verified; deploy only on green.
- **Routing** is the hard case (SSE + overlay + tap-capture + persist) — last, isolated, QA'd.
- **`entry-point` churn** — moving to barrels touches many imports; do it per-slice, not all at once.
- The `MapEngine` interface must track the WASM bindings — modest upkeep; it's also the engine's doc.
