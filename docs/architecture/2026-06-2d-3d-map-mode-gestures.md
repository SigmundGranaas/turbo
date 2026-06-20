# 2D / 3D map mode — gesture & camera design

Status: design (not yet implemented). Target: Android **wgpu/turbomap** engine only.
Date: 2026-06-18.

## Revision 2026-06-20 — unified gesture model (SUPERSEDES the orbit model below)

The 1-finger-orbit model below was reworked after device testing (it felt wrong: a
shaky finger spun the map, and pinches drifted/rotated). The shipped grammar:

- **One finger → pan** in BOTH 2D and 3D (slop-gated against jitter).
- **Two fingers → zoom (pinch) OR free orbit**, pivoting about the gesture **centroid**.
  No two-finger pan (one finger pans), so a pinch can't slide the map around.
- **One intent per gesture** (`lockTwoFingerAxis`): a two-finger gesture commits to
  **zoom** XOR **orbit**. Zoom accumulates against `ZOOM_GATE_LEVELS`; orbit engages on
  a twist (`ROTATE_GATE_DEG`) *or* a vertical drag (`TILT_GATE_DP`). All signals are
  summed as **signed net** movement so a twist's incidental wobble can't falsely win
  zoom. Whichever crosses first wins; the other stays suppressed — a pinch is a clean
  zoom, a non-pinch is a clean orbit.
- **Orbit is free rotation**: once orbiting, the bearing follows the twist AND (in 3D)
  the pitch follows the vertical drag, **together** — you look around in every direction
  at once, not locked to a single axis.
- Zoom pivots via `nativeZoomAround` (focus-anchored); rotate/tilt via
  `nativeOrbitAround(dBearing, dPitch, focusX, focusY)` about the centroid. Bearing
  sign turns the map *with* the fingers.

The 2D/3D mode flag now only decides whether two-finger **tilt** is allowed.
Everything below is the superseded original design, kept for history.

## Goal

Two explicit map modes the user toggles between, with different gesture semantics:

- **2D mode** — exactly today's behaviour (top-down, 1-finger pan, pinch zoom).
- **3D mode** — 1-finger drag *orbits* the camera around a pinned focal point
  (the user's location); two fingers translate (pan); pinch zooms. Tilt is free.

## Locked decisions

1. **Engine scope: wgpu/turbomap only.** Only that engine has pitch +
   `rotated_around`/`pitched_around` and the 3D cloud path. The MapLibre engine
   and iOS (MapKit) stay 2D. **Turning 3D on implies/forces the wgpu engine**
   (the 3D toggle is hidden/disabled on the MapLibre path).
2. **Orbit pivot = user location while following/on-screen; re-pins to screen
   centre once a two-finger pan breaks follow.** Recentre/follow re-pins to the
   dot.

## Mode model

- `MapMode { TwoD, ThreeD }` in `MapUiState` (default `TwoD`), persisted in
  settings next to `experimentalWgpuMap`.
- Toggle: a small segmented 2D/3D control on the map (near the compass).
- `focalLocation`: `userLocation` when following / on-screen, else `null`
  → pivot falls back to screen centre.

## Gesture map

| Input | 2D mode (= today) | 3D mode (new) |
|---|---|---|
| 1-finger drag | Pan (translate centre) + fling | **Orbit around pinned focal point**: horizontal Δ → bearing, vertical Δ → pitch |
| 2-finger drag (translate) | (part of pinch) | **Pan** (translate centre) + fling |
| Pinch (spread) | Zoom around centroid | Zoom around focal point |
| 2-finger twist | Rotate bearing around centroid | folded into 1-finger orbit (twist optional / ignored) |
| Tap / long-press | Select / context menu | unchanged |
| Pitch | locked 0° | free `[0, MAX_PITCH_DEG=60]` |

## Orbit mechanism (the crux)

The Rust `Camera` already has the primitives (`camera.rs:151-204`):
`rotated_around(Δdeg, focusPx, vp)` and `pitched_around(Δdeg, focusPx, vp)` both
re-centre so `focusPx` stays over the same world point (`recenter_on_focus`).

Per move event of a 1-finger drag in 3D:
```
focusPx = on-screen pivot (toScreen(focalLocation), else screen centre)
camera  = camera.rotated_around(dx * K_BEARING, focusPx, viewport)
camera  = camera.pitched_around(-dy * K_PITCH,  focusPx, viewport)   // clamp [0,60]
```
Because the pivot is pinned, the location stays glued to its screen spot while
the world spins/tilts around it — the requested behaviour, with zero new camera
math.

`K_BEARING` ≈ 0.3 °/px, `K_PITCH` ≈ 0.25 °/px (tune on device). Up-drag tilts
toward horizon (increase pitch); horizontal drag spins bearing.

## Layer-by-layer changes (wgpu path)

1. **Gesture detector** — `core/turbomap-android/.../MapGestureDetector.kt:92`
   `detectMapGestures()`. Add a `mode: MapMode` param. In `ThreeD`:
   - 1 pointer → `onOrbit(dx, dy, focusX, focusY)`
   - ≥2 pointers → existing centroid-pan + spread-zoom (`onTransform`)
   In `TwoD`: unchanged. Keep the "pinch never pan-flings" invariant; add an
   orbit-fling (bearing/pitch momentum) only if it feels needed.
2. **Surface controller** — `TurbomapSurfaceController.kt:376`. Add
   `onOrbit(dBearing, dPitch, focusX, focusY)` → `nativeOrbitAround(...)`.
   `onGestureDown` still cancels in-flight animation.
3. **FFI + engine** — `turbomap-ffi` + `turbomap-engine`: add
   `nativeOrbitAround(handle, dBearing, dPitch, focusX, focusY, vw, vh)` that
   calls `camera.rotated_around(...).pitched_around(...)`. Mirrors the existing
   `nativeSetCloudGeoBounds` plumbing. (Core methods already exist.)
4. **MapEngine seam** — add `setPitch(Double)` / `pitch(): Double` and a mode
   hook. Implemented on `TurbomapMapEngine`; the MapLibre `TurboMap` impl can
   no-op / throw since the 3D toggle is gated to wgpu.
5. **Mode/UI** — `MapMode` in `MapUiState` + a `MapScreen` toggle; persist via
   `SettingsRepository`. On enter-3D ease pitch 0→45° (so it reads as 3D and the
   clouds get their side-reveal); on exit ease pitch→0 (and bearing→0).
6. **Focal point** — `MapViewModel` exposes `focalLocation` from `userLocation`
   + follow state; `MapScreen` passes it into the gesture binding so the
   detector can resolve `focusPx` each frame (re-pin to centre once panned away).

## Transitions & edge cases

- **2D→3D:** ease pitch 0→45°, pivot = user dot (or centre if no fix).
- **3D→2D:** ease pitch→0, bearing→0; restore 1-finger pan.
- **No GPS fix:** pivot = screen centre.
- **Two-finger pan in 3D:** breaks follow → pivot re-pins to screen centre until
  recentre/follow.
- **Mid-gesture toggle:** ignore until the active gesture ends (or cancel it).
- **MapLibre / iOS:** 3D toggle hidden/disabled; they stay 2D.

## Clouds synergy

3D mode drives `pitch>0`, which triggers the (now-fixed, commit `bc85fa0e`)
camera-ray cloud parallax — the side-reveal appears exactly when the user tilts.
No extra work; just validate the rake reads well at the default 45°.

## Phasing

- **P1 — Camera FFI:** ✅ DONE (commit 55818fde). `nativeOrbitAround` in
  turbomap-ffi (engine already had `rotate_around`/`pitch_around`) + a core test
  that a combined rotate+tilt keeps the pivot world-point fixed.
- **P2 — Gesture routing:** ✅ DONE (commit 0c07e625). `MapGestureMode` in
  `detectMapGestures` (mode sampled once per gesture), `onOrbit` →
  `nativeOrbitAround`, `threeDMode` param on `TurbomapMapView`, orbit focus from
  `engine.toScreen(userLocation)` with the off-screen→centre re-pin rule.
- **P3 — Mode state + toggle UI:** ✅ DONE (commit de0a4a83).
  `MapUiState.threeDMode` (session, NOT persisted yet) + setter; 2D/3D rail
  toggle shown only when the wgpu engine is active; enter/exit pitch ease 0↔45°
  via `nativeEasePitch`. `ORBIT_*_DEG_PER_PX` = 0.30 / 0.25 (untuned).
- **P4 — Polish + device QA:** PENDING (device-only). Tune
  `ORBIT_BEARING/PITCH_DEG_PER_PX`, orbit direction/sign, fling feel; verify
  follow + recording interaction in 3D (orbit shifts centre via
  `recenter_on_focus` — confirm it doesn't fight the follow camera); two-finger
  pitch on hardware. Cloud rake at the default 45° tilt is already validated via
  the desktop scene (the pitch-3D parallax fix, commit bc85fa0e). Optional
  follow-ups: persist `threeDMode`; orbit-fling (bearing/pitch momentum).

## Status

P1–P3 implemented + gated green (cargo test, `:app:assembleDebug`, `:feature:map`
unit tests, detekt across designsystem / turbomap-android / feature:map). To try
it: enable the experimental wgpu map in Settings → a 2D/3D (ViewInAr) button
appears in the map control rail. Not yet device-QA'd.

## Out of scope (now)

MapLibre 3D, iOS 3D, terrain-relief-aware orbit (pivoting about ground
elevation), pitch-dependent zoom limits.
