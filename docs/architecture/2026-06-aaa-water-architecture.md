# AAA Water — Architecture & Scoping Doc

**Status:** scoping / architecture (no code yet)
**Date:** 2026-06-24
**Supersedes:** `2026-06-aaa-water-scope.md`, `2026-06-aaa-water-implementation-plan.md` (earlier, narrower attempts)
**Owner sign-off model:** reference-driven, on-device (see §9)

---

## 0. Why this document exists

Realistic water has been attempted ~10 times and failed the same way every time:
a hand-rolled procedural normal-map (sum-of-sines / `exp(sin)` / domain-warped
noise) tuned by eyeballing one screenshot at a time. Each "fix" traded one
artifact for another — foam-cells → dead-flat → tiling grid → white-wash →
flat — with **no net gain in fidelity**, because:

1. **Wrong tool.** A procedural normal map cannot be anti-aliased correctly
   across map zooms (no mip chain), is periodic (tiles into a grid when zoomed
   out), and carries no real wave *shape* — so it never reads as a cinematic
   ocean.
2. **No fixed target.** Tuning chased whatever frame was on screen, so the
   target moved every iteration and the work oscillated.
3. **Shipped blind.** Changes went out after inspecting frames that couldn't
   show the defect (top-down, or faded), so regressions weren't caught.

This document fixes all three: it commits to the **right class of technique**
(a real spectral/FFT ocean whose outputs are *textures*, hence properly
mip-filtered and non-tiling), pins a **fixed visual target** with a
**reference-driven acceptance harness**, and defines a **phased plan where
nothing ships until it matches a pinned reference at a fixed camera**.

The brief: *"detail how we achieve all of the details that make it triple-A and
make it feel real… architect this properly."* This doc is that.

---

## 1. Locked decisions (the spec)

Resolved with the owner via a structured design interview. These are fixed; the
rest of the doc elaborates *how*.

| # | Decision | Locked answer | Consequence |
|---|----------|---------------|-------------|
| 1 | **Visual target** | **Cinematic game ocean** (Uncharted-4 / AC-Black-Flag tier): real displaced wave geometry, visible rolling swells, crest foam, spray. | Mandates real geometry displacement + a foam/spray system, not a normal map. |
| 2 | **Zoom envelope** | **Alive at ALL zooms** (incl. regional z11–13), **zero tiling/grid artifacts.** Visible motion everywhere is valued over strict physical wave scale at distance. | The #1 hard requirement. Forces a multi-scale, mip-filtered, texture-based wave field. *Never fade-to-flat again.* |
| 3 | **Technique / perf** | **Real runtime FFT (Tessendorf) ocean** via a new compute pipeline. **Pixel 9 Pro @ 60 fps**, ~2–4 ms GPU water budget. | Adds compute + storage textures to the engine; needs a baked fallback where compute is unavailable (WebGL2). |
| 4 | **Reflections** | **Hybrid**: screen-space reflection (SSR) for on-screen detail + DEM heightfield-march for off-screen mountain silhouettes + analytic sky/cloud fallback. | **Mandates the frame restructure** (scene colour + linear depth as sampleable textures). |
| 5 | **Scope** | **Sea/fjord + lakes** = one system, two presets (sea = full swell; lake = calm/glassy, mirror-dominant). **Rivers stay flat** (flow is out of scope). | One ocean model; a `SeaStatePreset` per water-body class. |
| 6 | **Realism cues (all v1)** | Foam (crest + shore), depth colour + refraction, subsurface scattering, **and spray/mist particles**. Reflection already in. | Every cue is a designed subsystem below; nothing deferred to "later" except §11. |
| 7 | **Shallowness/depth** | **DEM-extrapolated + edge-distance** for v1. **Real Kartverket bathymetry = phase-2** accuracy upgrade. | A `Shallowness` field derived from existing data; no new data pipeline in v1. |
| 8 | **Acceptance** | **Reference-driven, side-by-side**, per-cue pass/fail, on-device sign-off against pinned references. | The anti-circling backbone (§9). |

**Target platforms:** Android (Pixel 9 Pro, Vulkan) is the binding target.
macOS desktop (Metal) is the dev/tuning surface. Web (WebGPU) gets the full path;
**WebGL2 fallback gets the baked tier** (§7.5).

---

## 2. Domain model (ubiquitous language)

One vocabulary, used identically in code, comments, this doc, and review. (Per
the project's domain-modeling practice.)

- **Ocean Field** — the analytic, world-space description of the sea surface at a
  given time: for any world (x,y) it yields a **Displacement** and a **Surface
  Normal**. Produced by the **Spectral Simulation**. Tile-agnostic and infinite;
  it is *sampled*, never *placed*.
- **Spectral Simulation** — the Tessendorf FFT engine. Turns a wind/wave
  **Sea State** into the Ocean Field each frame via an inverse FFT on the GPU.
- **Sea State** — the physical forcing: wind speed/direction, significant wave
  height, dominant swell direction, fetch. Sourced live from the MET forecast
  (already wired) or a preset. One Sea State → one **Spectrum**.
- **Spectrum** — the directional wave energy distribution (JONSWAP × directional
  spread). Defines the initial complex amplitudes `h0(k)`. Recomputed only when
  the Sea State changes (cheap, infrequent).
- **Cascade** — one FFT patch at a fixed world scale (e.g. 512 m / 64 m / 8 m).
  We run **2–3 cascades** and sum them; this is what makes the field detailed at
  every zoom and visually non-repeating (mutually-prime patch sizes).
- **Displacement** — per-point offset of the surface: horizontal "choppiness"
  (x,y) + vertical height (z). Applied to the **Water Grid** vertices.
- **Jacobian / Foam Mask** — the fold/pinch measure of the Displacement; where it
  goes negative the surface is overlapping → a breaking crest → **crest foam**.
- **Water Grid** — the per-tile, refined, draped triangle mesh (already built)
  whose vertices are displaced by the Ocean Field. Distinct from the Ocean Field
  (which is the *data*); the grid is the *geometry that samples it*.
- **Shallowness** — scalar 0 (deep) → 1 (at shore) for any water point. Drives
  depth colour, refraction, foam, and wave shoaling. v1 = `f(DEM under-water
  slope, distance-to-shore)`.
- **Reflection Probe** — the hybrid that answers "what is mirrored here?": SSR →
  DEM-march → sky, in that priority.
- **Scene Colour / Scene Depth** — the resolved opaque frame (colour + linear
  eye-depth) as sampleable textures; the inputs SSR and refraction read.
- **Sea State Preset** — named tuning of the Sea State per water-body class:
  `Open`, `Fjord`, `Lake`. Selected per water polygon at tessellation time.

---

## 3. The core insight — why FFT *textures* fix the unsolved problem

Decision #2 (alive at all zooms, no grid) is the requirement every past attempt
failed. The fix is not a cleverer procedural function; it is a change of
representation:

> **The Ocean Field is produced as a small set of GPU textures (per cascade:
> displacement + normal + foam). Textures have mip chains and anisotropic
> filtering. That is exactly the machinery that makes a repeating signal look
> correct at every distance — the thing procedural noise in a fragment shader
> cannot do.**

Three properties fall out, each killing a specific past failure:

1. **No zoom-out grid / no aliasing.** When the camera pulls back, the cascade
   textures are sampled at coarser mips (and anisotropically along the view).
   Fine ripples *average down* smoothly instead of aliasing into a lattice —
   the same reason a brick texture doesn't shimmer at distance. The "chess grid"
   was sub-pixel procedural waves with no mip; this removes the cause.
2. **No visible tiling.** Each cascade tiles at its own patch period (512/64/8 m).
   Summed with mutually-prime periods + the largest cascade carrying the
   low-frequency swell, the *combined* field's repeat is far larger than any
   on-screen extent, and the eye cannot lock onto it.
3. **Real wave shape + motion at all zooms.** The largest cascade is a real
   long swell (hundreds of metres) — visible and moving even at z11–13. The fine
   cascades add chop you only resolve up close. Same field, correct detail per
   zoom, always animated. This satisfies "alive everywhere" *without* the
   wrong-scale fakery that broke things.

This is why a spectral ocean is not just "nicer" — it is the representation that
makes the locked requirements *physically achievable*. Everything else in the
doc hangs off it.

---

## 4. What makes it feel real — cue by cue (the AAA detail list)

Each cue: what it is, the exact technique, the inputs it consumes, and the
pass/fail line used at sign-off (§9). This is the heart of "detail how we
achieve all the details."

### 4.1 Wave shape & motion (the spine)
- **What:** large rolling swells you can watch move, with fine chop riding on
  them; direction follows the wind; bigger in a storm, glassy in calm.
- **How:** Tessendorf FFT (§5). Vertical height + horizontal (Gerstner-style)
  choppiness from the displacement field, summed over cascades, sampled at world
  XY in the **Water Grid vertex shader** → real displaced geometry. Animated by
  evolving the spectrum in time (`ω(k)=√(gk·tanh(kD))`, with `D` from
  Shallowness so waves *shoal* near shore).
- **Inputs:** Sea State (MET forecast), Shallowness (depth `D`), time.
- **Pass/fail:** at a fixed camera the crests visibly translate frame-to-frame
  in the wind direction; silhouette against the horizon shows real bumps at
  z16 tilt; no static or repeating pattern at z11.

### 4.2 Reflections (the biggest "real" tell on a fjord)
- **What:** the surrounding mountains, sky, and clouds mirrored on the surface,
  rippling with the waves; the mirror present even for peaks above the view.
- **How:** **Reflection Probe** = priority chain per fragment, using the wave
  normal to build the reflected ray:
  1. **SSR** — DDA march the reflected ray against **Scene Depth**, sample
     **Scene Colour** on a hit (real basemap/terrain/clouds, exact).
  2. **DEM-march** — on SSR miss/off-screen, march the 256² heightfield (already
     built) for the real mountain silhouette, shaded by the shared sun.
  3. **Sky** — on both misses, the analytic sky (shared with the sky pass) +
     sun halo.
- **Inputs:** Scene Colour, Scene Depth (frame restructure §6), heightfield,
  sky globals, wave normal.
- **Pass/fail:** at a fjord, z16 tilt — the actual peak shapes appear inverted
  on the water and wobble with the chop; rotating the camera keeps them
  consistent; no hard seam where SSR hands off to DEM-march.

### 4.3 Depth colour & refraction
- **What:** turquoise, see-through shallows that reveal the seabed (refracted);
  saturated navy in deep water; a believable colour gradient between.
- **How:** Beer–Lambert absorption over **Shallowness**; in shallows, sample
  **Scene Colour** at a normal-perturbed (refracted) UV to show the seabed
  through the surface, attenuated by depth; blend to the deep tint by absorption
  length. Physically-grounded palette (not the pale style colour darkened — that
  was a past failure).
- **Inputs:** Shallowness, Scene Colour, wave normal, view vector.
- **Pass/fail:** near a sandy shore the bottom is visible and bends with ripples;
  deep water is opaque saturated blue; the transition is smooth, not banded.

### 4.4 Foam — crest + shore
- **What:** white breaking foam on steep/overlapping crests; persistent,
  animated foam where water meets land.
- **How:** **crest foam** from the FFT **Jacobian** (fold detector) — accumulates
  where waves pinch, advected and decayed over time for streaky persistence.
  **Shore foam** from the DEM waterline band (already prototyped), widened by
  wave run-up (Sea State) and wobbled in time.
- **Inputs:** Jacobian (from FFT), Shallowness/DEM, Sea State, time.
- **Pass/fail:** whitecaps appear only on the steepest crests and dissipate; a
  living foam line hugs the coast and breathes; both scale with sea state.

### 4.5 Subsurface scattering
- **What:** the glowing teal/green light through the back of a sunlit swell.
- **How:** back-lit term — `pow(max(dot(view, -sun_through_wave),0), k)` gated by
  wave height and `sun·up`, tinted by the shallow-water scatter colour; strongest
  on tall crests with the sun behind them.
- **Inputs:** wave height/normal, sun dir, view vector.
- **Pass/fail:** with the sun low behind a swell, crest edges glow translucent
  green; absent in flat/backlit-absent conditions.

### 4.6 Sun glitter
- **What:** the sparkling specular path toward the sun — scattered points, not a
  white sheet.
- **How:** tight HDR specular (high exponent) on the *fine-cascade* normal so
  micro-facets spark; HDR value > 1 feeds the existing bloom → bloom turns it
  into sea-sparkle. Mip-correct (fine cascade has mips) so it doesn't fizz/alias
  at distance.
- **Inputs:** fine-cascade normal, sun dir, half-vector, HDR/bloom (existing).
- **Pass/fail:** a scattered, animated glitter path toward the sun; never a solid
  white blob; doesn't shimmer-alias when zoomed out.

### 4.7 Spray / mist (near-surface only)
- **What:** particles flung off breaking crests; cinematic when the camera is
  near big surf.
- **How:** GPU particle system seeded from the foam mask (crest-break events),
  billboarded, lit by sun, additive, fading; **density gated by camera altitude**
  (≈0 at map zoom, where it's sub-pixel; full near the surface). New subsystem
  (§5.6); scheduled last (lowest map-relevance, highest cost).
- **Inputs:** foam mask, camera altitude, sun.
- **Pass/fail:** at z18 grazing a storm sea, spray lifts off whitecaps and
  drifts; invisible/no-cost at z12.

### 4.8 Lighting / time-of-day / sea-state truth
- **What:** the water obeys the real sun (colour, glitter direction, night) and
  the real forecast (calm vs storm), so it matches the world.
- **How:** reuse the existing real-clock `SunPosition` + `sun::atmosphere`
  palette (shared with sky/terrain) and the live `WaterConditions::from_forecast`
  wiring (both already plumbed Rust↔Android). Night = low sun → dark water +
  sky/star reflection; storm = high `wave_amp`/whitecap.
- **Pass/fail:** at golden hour the glitter path tracks the real sun azimuth;
  a forecast storm visibly roughens the sea.

---

## 5. Technical architecture

### 5.1 System overview

```
 MET forecast ──▶ Sea State ──▶ Spectrum h0(k)         [CPU, on change]
                                   │
        time ────────────────────▶│
                                   ▼
              ┌─────────────────────────────────────────┐
              │  Spectral Simulation  (COMPUTE, /frame)  │
              │  per cascade: evolve h(k,t) → IFFT →     │
              │   • displacement tex (xy choppy, z high) │
              │   • normal tex                           │
              │   • foam/Jacobian tex     (all mipmapped)│
              └─────────────────────────────────────────┘
                                   │  (Ocean Field = these textures)
                                   ▼
  Opaque pass ─▶ resolve ─▶ Scene Colour + Scene Depth (linear)
                                   │
                                   ▼
              ┌─────────────────────────────────────────┐
              │  Water pass (RENDER)                     │
              │  VS: sample displacement → displace grid │
              │  FS: normal+foam from Ocean Field;       │
              │      Reflection Probe (SSR/DEM/sky);     │
              │      refraction+depth colour (Scene tex);│
              │      SSS; sun glitter (→ HDR/bloom)      │
              └─────────────────────────────────────────┘
                                   │
                          composite → bloom/tonemap → clouds → screen
                                   ▲
                    Spray particles (near-surface) draw here
```

### 5.2 Spectral Simulation (the FFT ocean) — new `render/ocean/`

- **Spectrum:** JONSWAP (wind-sea, fetch-limited) × a directional spreading
  function (cosine-2s) about the dominant swell direction. Parameterised from
  the Sea State (`wind_speed_ms`, `wind_from_deg`, `wave_height_m`,
  `wave_from_deg` — all already delivered). Build the initial complex field
  `h0(k)` once per Sea-State change on the CPU (or a one-shot compute), upload as
  a texture per cascade.
- **Time evolution + IFFT (compute, per frame):**
  - Evolve `h(k,t) = h0(k)e^{iωt} + h0*(-k)e^{-iωt}`, `ω(k)=√(g·k·tanh(k·D))`.
    `D` (depth) from Shallowness so the dispersion shoals near shore.
  - 2D inverse FFT via the standard two-pass butterfly (rows, then columns) in
    compute, with a precomputed butterfly/twiddle texture. Output displacement,
    plus slope→normal and the Jacobian→foam, into **storage textures**.
  - **Cascades:** 3 patches — `L₀≈512 m`, `L₁≈64 m`, `L₂≈8 m`, each `N=256²`
    (tunable; drop to 2×128² on the fallback tier). Mutually-prime-ish sizes to
    avoid coherent tiling.
  - **Mips:** generate mip chains for the output textures (blit downsample) so
    distance sampling is anti-aliased — the crux of §3.
- **wgpu enablement:** the device is currently requested with `Features::empty()`
  + `downlevel_defaults()` (GLES-safe). Compute + storage textures are available
  on Android Vulkan, Metal, and WebGPU. Change: request compute capability +
  required storage-texture limits, **guarded by adapter capability** so the
  raster path still initialises where compute is absent (→ baked fallback §7.5).
  Keep `LowPower`? Re-evaluate; one mobile GPU, so minor.
- **Cost (Pixel 9 Pro est.):** 3 cascades × (evolve + 2-pass IFFT for
  displacement & normal) ≈ 1–1.8 ms; mip-gen ≈ 0.2 ms. Within budget; the
  fallback tier halves cascades/resolution.

### 5.3 Frame restructure (required by SSR + refraction)

Today: one monolithic render pass (sky, floor, raster, **water**, vector,
hillshade, tubes, icons, text, markers) → resolve → bloom/tonemap → clouds
(`map.rs::render`). Water reads nothing.

New (only when realistic water is **on** — the flag selects the path, so the
existing path is untouched and risk is isolated):

1. **Opaque pass** — sky, floor, raster, vector (no water), hillshade, tubes →
   MSAA colour + MSAA depth. Resolve colour → **Scene Colour** (`Rgba16Float`,
   single-sample, `TEXTURE_BINDING`).
2. **Depth-linearise pass** — fullscreen; read MSAA depth sample 0
   (`texture_depth_multisampled_2d`), write linear eye-depth → **Scene Depth**
   (`R32Float`, `TEXTURE_BINDING`). *(New target; today no linear depth exists.)*
3. `copy_texture_to_texture(Scene Colour → Composite)` so non-water pixels keep
   the opaque image.
4. **Water pass** — colour = Composite (`LoadOp::Load`), depth = MSAA depth
   **read-only** (so terrain occludes water). Binds Scene Colour + Scene Depth +
   Ocean Field + heightfield. Draws the displaced Water Grid.
5. **Overlay pass** — icons, text, markers into Composite (moved out of the old
   single pass).
6. **Post** — bloom/tonemap read Composite → screen; clouds; **spray particles**
   composited near-surface.

Risk note: this is the most invasive change. Mitigation: it lives entirely
behind the `realistic_water` flag; flag-off keeps the current single pass
verbatim. The flag already exists end-to-end (Rust + Android FFI).

### 5.4 Reflection Probe (SSR + DEM-march + sky)
- SSR: DDA along the reflected ray in screen space against Scene Depth (~24–32
  steps + a binary refine), sample Scene Colour on a hit; thickness test to
  reject false hits. Single-sample (samples resolved textures) → feather water
  edges to hide MSAA loss.
- Off-screen / miss → reuse the existing heightfield-march (mountain silhouette,
  sun-shaded) → on its miss, analytic sky. Smooth confidence-blend between tiers
  (no hard seam — a past artifact).

### 5.5 Refraction, depth colour, foam, SSS
As §4.3–4.6. All read the Ocean Field (normal/foam), Shallowness, and Scene
Colour/Depth. The **Shallowness** field: a small camera-centred texture (reuse
the heightfield assembly path) computing `saturate(blend(under-water DEM slope,
distance-to-shore))`; feeds depth `D` back into §5.2 for shoaling.

### 5.6 Spray particles — new `render/spray/`
GPU particle pool; spawn compute reads the foam mask and emits at crest-break
cells; update integrates gravity/drag; draw as sun-lit additive billboards into
Composite. Global density × `f(camera altitude)` → zero cost at map zoom. Last
to build.

### 5.7 Bind-group budget (hard cap = 4 groups)
The water FS is binding-heavy; `max_bind_groups=4` is a real mobile limit. Plan
(bindings-per-group, not group count, is the slack — downlevel allows ~16
sampled textures/stage):
- **group 0** camera (shared) · **group 1** per-tile (shared) · **group 2**
  terrain DEM for draping (shared).
- **group 3** = water uniforms + Ocean Field (pack to **2 textures/cascade**:
  RGBA = displacement.xyz+foam, RG/RGBA = normal(+Jacobian); 3 cascades = 6) +
  heightfield + Scene Colour + Scene Depth + samplers ≈ 9 sampled textures —
  within the 16 cap. Vertex stage samples only displacement (subset). Documented
  exactly in the implementation PR.

### 5.8 LOD & the Water Grid
Reuse the refined per-tile Water Grid (already built). Vertex density vs cascade
detail: tessellate finer only within N metres of the camera (screen-space error
target); displacement sampled from cascades by world XY with mip bias from
triangle size. Far tiles: coarse grid + coarse mips = smooth, cheap, no grid.

### 5.9 Performance budget (Pixel 9 Pro, 16.6 ms frame; water ≤ ~4 ms)
| Stage | Est. ms | Notes |
|---|---|---|
| Spectrum evolve + IFFT (×3 cascades) | 1.0–1.8 | compute; fallback halves it |
| Mip generation | ~0.2 | blit downsample |
| Depth-linearise + copy | ~0.3 | fullscreen + copy |
| Water grid draw (VS displacement) | 0.3–0.6 | bounded vertex count |
| Water FS (SSR march + reflection + refraction + foam + SSS) | 1.0–1.8 | SSR march is the cost driver; step-count is the tuning knob |
| Spray | 0–0.5 | gated to near-surface |
| **Total** | **~3–5 ms** | tier-down (fewer cascades, shorter SSR, no spray) for thermal/mid-range |

A **quality tier** (High/Med/Low) selected by adapter + thermal: cascades 3→2,
FFT 256→128, SSR steps 32→16→off (DEM-march only), spray on→off.

---

## 6. Data inputs (all already wired unless noted)
- **Sea State:** MET oceanforecast → `ConditionsRepository` → `MapViewModel.seaState`
  → `nativeSetWaterConditions` → `WaterConditions::from_forecast`. Reuse; feed
  the **Spectrum**, not just the old normal-map amplitude.
- **Sun/time:** real-clock `nativeSetSunTime` → NOAA `solar_position` →
  `SunPosition`; shared `sun::atmosphere`. Reuse verbatim.
- **Shallowness:** derived in-engine from DEM + shoreline (v1). **Phase-2:** real
  Kartverket *Dybdedata* bathymetry through a DEM-like tile pipeline.
- **Scene Colour/Depth:** produced by the frame restructure (§5.3).
- **Satellite/basemap seabed:** already a mip-chained GPU texture
  (`TextureCache`); visible through refraction via Scene Colour.

---

## 7. Platforms & fallback
- **Android (Pixel 9 Pro, Vulkan):** full path. Binding perf target.
- **macOS (Metal):** full path; primary dev/tuning surface (interactive app).
- **Web — WebGPU:** full path (compute available).
- **Web — WebGL2 (fallback) & any no-compute adapter (7.5):** **baked tier** —
  pre-bake the FFT cascades offline into looping tiling-aware displacement/normal/
  foam texture arrays; sample at runtime (no compute). Same water *shader/look*;
  only the Ocean Field *source* swaps (runtime FFT vs baked sampler). This makes
  the field-source a clean seam (`OceanFieldSource` trait: `Fft` | `Baked`).

---

## 8. Module / file plan (where it lives)
- `render/ocean/mod.rs` — `SpectralSimulation`, `Cascade`, `OceanFieldSource`
  (`Fft`|`Baked`), spectrum build, IFFT compute, mip-gen. **New.**
- `render/ocean/ocean_compute.wgsl` — spectrum-evolve + butterfly IFFT + foam.
  **New.**
- `render/water.rs` / `water_shader.wgsl` — rewrite FS/VS to consume the Ocean
  Field + Reflection Probe + refraction (replaces the procedural normal map).
- `render/targets.rs` — add Scene Colour, Scene Depth, Composite. **(restructure)**
- `render/frame.rs` + `map.rs::render` — the multi-pass restructure, flag-gated.
- `render/depth_resolve.rs` (+ shader) — MSAA depth → linear. **New.**
- `render/spray/` — particle subsystem. **New, last.**
- `render/shallowness.rs` — Shallowness field assembly (reuse heightfield path).
  **New.**
- `tessellate.rs` — keep the refined Water Grid; carry Sea-State preset per body.
- Device setup (`surface.rs`, `gpu.rs`, `turbomap-web`) — request compute/storage
  with capability guard + tier selection.
- FFI/Android — reuse existing toggle + conditions; add a quality-tier setter if
  needed.

---

## 9. Acceptance & verification — the anti-circling system

This is non-negotiable and built **before** tuning. The water never circles again
because the target is pinned on screen next to the render.

### 9.1 Reference set (`apps/turbomap/refs/water/`)
A fixed set of **target images** + an exact **camera + Sea State + time** manifest
per image. Initial set (extend as needed):
| id | scene | camera | sea state | judges |
|---|---|---|---|---|
| `fjord_clear_z16` | enclosed fjord, peaks around | z16, tilt 60°, sun low | calm | reflection, depth colour, glitter |
| `open_moderate_z14` | open sea | z14, tilt 45° | moderate | swell shape, motion, foam |
| `lake_glassy_z15` | mountain lake | z15, tilt 50°, sun mid | calm/lake preset | mirror, stillness |
| `storm_z16` | open sea | z16, tilt 55° | storm | whitecaps, spray, chop |
| `shore_z17` | sandy + rocky shore | z17, tilt 40° | low | shore foam, refraction/seabed |
| `regional_z11` | coast from altitude | z11, tilt 30° | moderate | **no grid, no white-wash, still alive** |
| `night_z15` | coast at night | z15, tilt 50°, sun down | calm | dark water, sky/star reflection |
References are real fjord/ocean photos and/or cinematic-ocean stills chosen with
the owner. They are the **fixed yardstick**.

### 9.2 Comparison harness (extend `examples/scenario.rs`)
For each ref id: render OUR water at the **exact** matched camera/sea-state/time
and emit a **side-by-side composite PNG** (ours | reference) into
`target/water-cmp/<phase>/<id>.png`. Uses the real-data test-bed already built
(kart-api water + Esri satellite + sun controls + JPEG decode). The harness is
the regression surface; it runs headless.

### 9.3 Per-cue checklist (per ref, per phase)
A markdown checklist scored each phase: reflection present+correct · depth
gradient + visible seabed in shallows · crest foam on steep crests only · shore
foam line alive · waves visibly move in wind dir · **no tiling/grid at z11** ·
glitter scattered (not a sheet) · colour matches ref within tolerance · perf ≤
budget. A phase is **not done** until its checklist passes for its ref(s).

### 9.4 On-device sign-off
`tools/water_shots.sh` (built) captures the same fixed views on the Pixel; the
owner signs off **on device, against the pinned reference**, per phase. Look is a
device decision; the harness guards geometry/structure/perf.

### 9.5 Automated regression gates (CI/harness)
- **NaN/finite gate** (existing fuzz discipline) — no NaN reaches the GPU.
- **No-tiling test** — autocorrelation of a flat-sea patch must have no sharp
  secondary peak at a tile/patch period (catches grid regressions numerically).
- **Perf gate** — GPU ms within tier budget (timestamp queries, already present
  on desktop).
These run every build; they catch the *structural* failures (the ones that kept
recurring) without a human.

---

## 10. Phased plan (each phase = a reference checkpoint)

Each phase ends at a **specific reference it must match** before merge. No phase
merges on vibes.

- **P0 — Verification scaffold + frame restructure.** Build the reference set,
  comparison harness, checklist, no-tiling + perf gates. Implement the
  opaque→resolve→water multi-pass behind the flag with the *current* water
  shader (prove the restructure is visually identical to today). **Checkpoint:**
  harness diff vs current build ≈ 0; gates green. *(Nothing visual changes yet —
  this is the safety net that makes the rest non-circular.)*
- **P1 — Spectral Simulation + displaced geometry.** Compute FFT (1 cascade
  first, then 3), mipped Ocean Field, vertex displacement on the Water Grid,
  basic lit surface. **Checkpoint:** `open_moderate_z14` shows real moving
  swells; `regional_z11` no grid/aliasing (the historically-broken case proven
  fixed first).
- **P2 — Reflection Probe.** SSR + DEM-march + sky, fed by Scene Colour/Depth.
  **Checkpoint:** `fjord_clear_z16` mirrors the real peaks; no SSR/DEM seam.
- **P3 — Depth colour + refraction + Shallowness.** **Checkpoint:** `shore_z17`
  seabed visible/refracted in shallows; deep water saturated; smooth gradient.
- **P4 — Foam (crest Jacobian + shore) + SSS + glitter.** **Checkpoint:**
  `storm_z16` whitecaps on crests + shore foam alive; `fjord_clear_z16` glitter
  scattered.
- **P5 — Lake preset + night + sea-state truth end-to-end.** **Checkpoint:**
  `lake_glassy_z15`, `night_z15` pass; live MET storm visibly roughens sea.
- **P6 — Spray particles + perf tiering + device ship.** **Checkpoint:**
  `storm_z16` at z18 grazing shows spray; all tiers within budget on device;
  full on-device owner sign-off across the reference set.
- **P7 (phase-2) — real Kartverket bathymetry** (documented upgrade; not v1).

Each phase: implement → harness side-by-side vs its ref → checklist → on-device
sign-off → merge. Gates run throughout.

---

## 11. Explicit non-goals (v1)
- Rivers / directional flow (stay flat).
- Boat/character wakes & interactive displacement.
- Underwater caustics / full underwater rendering.
- Real bathymetry (phase-2, §10 P7).
- Buoyancy/physics coupling.

---

## 12. Risks & mitigations
| Risk | Mitigation |
|---|---|
| Frame restructure breaks existing rendering | Entirely behind `realistic_water` flag; flag-off path untouched; P0 proves pixel-parity before any water change. |
| Mobile perf / thermal | Quality tiers (cascades, FFT size, SSR steps, spray); hard ms budget + perf gate; LowPower→reassess. |
| `max_bind_groups=4` | Pack Ocean Field into 2 tex/cascade; exact binding table in PR; verified against downlevel limits. |
| Tiling returns (the recurring failure) | Mutually-prime cascade periods + mip/aniso filtering + automated autocorrelation gate. |
| WebGL2 has no compute | `OceanFieldSource::Baked` fallback (offline-baked cascades). |
| SSR misses / single-sample edges | DEM-march + sky fallback; edge feather; thickness test. |
| Circling again | The whole of §9 — fixed references, side-by-side harness, per-cue checklist, phase checkpoints. |

---

## 13. Open questions for the owner (pre-P0)
1. **Reference images:** do you have specific fjord/ocean stills you want as the
   yardstick, or should I assemble a candidate set for your approval first?
2. **Quality-tier UX:** auto-detect only, or also a user setting (High/Med/Low)?
3. **Lake "glassy" intensity:** mirror-perfect, or always a little life?
4. **Night water:** reflect stars/moon (needs a night-sky source), or just dark +
   sky gradient?

---

*This doc is the contract. If the water starts circling again, the failure is a
process violation (shipped without matching a pinned reference at a checkpoint),
not a missing idea — and §9 is how we catch it.*
