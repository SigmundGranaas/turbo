# Android testing principles

**Date:** 2026-06-26 · **Scope:** `apps/android`

## The rule

A test must fail **only when a user-perceivable behaviour changes** — never when a
method is renamed or collaborators are reorganized. The "unit" is a unit of
*behaviour* (a capability the user cares about), exercised through its observable
boundary — a ViewModel's emitted state, a returned value, a callback the UI reacts
to — with fakes only at the true edges (network, GPS, renderer, clock).

If you can break a test by refactoring without changing what the user experiences,
it's testing mechanism, not behaviour.

## Smells (reject these in review)

1. Test names containing **`delegates` / `asks` / `calls` / `invokes` / `mirrors`** —
   they describe the implementation, not the outcome.
2. Assertions on a collaborator's **call count** (`fake.someCallCount == 1`) instead
   of the system's resulting state.
3. **One test class per production class, one test per public method** — coverage of
   "convenient methods" rather than user goals.
4. **Smoke / boot tests** — "the app launches", "the screen renders", "it doesn't
   crash". These validate nothing specific. There are **no smoke tests in this
   codebase**: a test either validates a specific user-visible behaviour, or it is
   removed. (Boot is covered for free — every behaviour E2E launches the app to get
   to the thing it actually asserts.)

The litmus: *would deleting this test lose coverage of something a user cares about?*
If the only thing it proves is "method A called method B," delete it (the real
behaviour is covered by a higher-level test) or reframe it to the observable outcome.

Worked examples of the reframe live in the git history of `SyncViewModelTest`
(call-count → "the screen shows syncing while a sync runs, then settles") and
`RecordingViewModelTest` (a faithful fake service so `stop()` asserts
`state.recording == false`, not `launcher.stops == 1`).

## The test pyramid

```
many   ── JVM unit              pure logic + ViewModels-with-fakes, named as outcomes
  │       Robolectric component  one composable / screen in isolation
 fat ──── HEADLESS E2E           the real app, no GPU — the source of truth for "does it work"
  │       Roborazzi (optional)   deterministic chrome snapshots (NOT the wgpu surface)
 thin ─── on-device E2E          a few specific user goals on real GPU (emulator)
few  ──── Rust render correctness  turbomap-sim + Lavapipe (deterministic, headless)
```

Render correctness belongs in the Rust/Lavapipe lanes — **do not** reintroduce
Android-level golden *pixel* tests of the map (they were removed as flaky).

## Headless E2E — how it works

The whole app runs under Robolectric with **no GPU** because the renderer is faked
at the one seam that needs a surface:

- `feature:map-core` exposes `LocalMapEngineOverride: CompositionLocal<MapEngine?>`
  (null in production → real wgpu surface). The map host (`MapScreen`) renders a
  no-op `Box` and drives the host with the override when it's present — this also
  makes the map work in Compose `@Preview`.
- Tests launch an `@AndroidEntryPoint HiltTestActivity` (debug source set) via
  `createAndroidComposeRule`, then `setContent { … TurboNavGraph() }` with a
  `FakeMapEngine` provided into `LocalMapEngineOverride`. `FakeMapEngine` records
  what the app asked the map to do (`lastFlyTo`, …) so tests assert *where the user
  ended up looking* — not that a method was called.
- Network is faked through Hilt: `FakeRemoteRepositoriesModule` (`@TestInstallIn`)
  binds the existing `Synthetic*` repos, so flows are deterministic and offline.
- `:app` needs `testOptions { unitTests { isIncludeAndroidResources = true } }` so
  Robolectric can load the merged manifest (resolve `HiltTestActivity`) and render.

See `app/src/test/.../e2e/` for the harness and `SearchCentersMapE2ETest`
("searching for a place centres the map on it") as the template.

## CI lanes

- `android_build.yml` — JVM unit + Konsist + **headless E2E** (`:app:testDebugUnitTest`)
  + `:app:assembleDebug`. No emulator. This is where new behaviour tests land.
- `android_ffi.yml` (Lane D) — wgpu engine on Lavapipe (software Vulkan).
- `android_device.yml` (Lane E) — real on-device GPU (emulator + KVM). On-device
  E2E goes here, sparingly — and each one must assert a *specific* user goal (e.g.
  "the recorded track is drawn on the map"), never "the app launches without
  crashing". A test that only proves the app comes up validates nothing; delete it.
- `turbomap_build.yml` — Rust engine correctness + `turbomap-sim`.

When adding a module with tests, add its `:…:testDebugUnitTest` to the unit lane in
`android_build.yml` (Gradle 9 fails the task on a module with *no* discovered tests,
so only list modules that actually have them).
