# AAA Water — detailed implementation + device-verification plan

Builds on `2026-06-aaa-water-scope.md`. This is the *executable* plan: exact files,
math, and — critically — **how each phase is rendered locally AND captured on the
device before it's committed**. The hard lesson from the normal-mapped attempt:
the desktop Metal golden did NOT predict the device GPU (the `fwidth` LOD and the
crest aliasing only showed on-device). So device verification is a first-class
deliverable, built in Phase 0 and run every phase.

All work lands on `main`, gated behind the **`waterMode`** rail toggle (already
shipped) → plumbed to a core `Map::set_realistic_water(bool)` flag. Off = today's
shader; on = the new path. Default off until Phase 4.

---

## Phase 0 — Device-verification harness + render foundation

### 0a. Deterministic device-capture (build this FIRST)

The blind iteration today came from not being able to put the device at a known
view on demand. Fix that:

- **Debug camera intent** (`MainActivity`, debug build only): an `adb`-launchable
  intent that sets camera `lat,lng,zoom,pitch,bearing` and forces `wgpu+3D+water`
  (+ optional `sun`, `time`). e.g.
  `am start -n …/.MainActivity -e water_view "67.30,14.40,16,70,20"`.
  Makes every test view reproducible without touching the screen.
- **`tools/water_shots.sh`**: `adb` wake → `settings put system screen_off_timeout
  1800000` → for each named view: launch the intent, wait for the cold-load trace
  to settle (`TurbomapTrace … pending=0`), `screencap`, pull to
  `target/water-shots/<phase>/<view>.png`. Also greps the last `PERF` line
  (fps / gpu_ms / mem) into a `metrics.txt`.
- **Debug HUD** (water-mode only): draw the trace `fps | gpu_ms | mem | water_tris`
  as text so a screenshot captures the budget, not just the look.

### 0b. The five fixed test views (acceptance set, used every phase)

| View | Camera | What it proves |
|------|--------|----------------|
| **V1 fjord-close** | Bodø coast, z16, pitch 72, bearing 20 | wave silhouette/parallax, glitter |
| **V2 coast-mid** | z13, pitch 55 | mountains reflected on water |
| **V3 regional-top** | z11, pitch 0 | calm + clean (no zigzag/moiré/clay) |
| **V4 mountain-lake** | a lake at elevation, z15, pitch 60 | water sits at `elev_m`, reflects |
| **V5 shoreline** | z17, pitch 78 | foam band + depth/refraction near shore |

Each view has a written **pass criterion** (e.g. V3: "uniform calm surface, zero
repeating pattern at 1:1 crop"). A phase isn't done until all relevant views pass
on-device AND the trace shows ≥ the fps floor (target 30 fps in water mode on the
Pixel 9 Pro; hard floor 24).

### 0c. Local golden parity

- Add **`water-preview` golden cameras** mirroring V1–V5 (GaussianTerrain + a
  flat-sea fixture) → dump to `target/water-smoke/`. Calibrate the synthetic
  scene scale so each preview's eye-distance (the LOD input) matches the device
  at that view (today the golden's `dist_m` was ~10× the device's → the LOD
  behaved differently). Calibration = one-time: read the device `dist_m` from a
  debug log at each view, set the golden camera to match.
- **Workflow rule (enforced):** edit shader → render the golden previews →
  *look at all five* → only then build/install → capture device V1–V5 → compare →
  commit. No commit without both.

### 0d. Render foundation

- **`render/targets.rs`**: add `linear_depth` (R32Float, `RENDER_ATTACHMENT |
  TEXTURE_BINDING`) and keep using the existing `hdr_resolve` (resolved HDR
  colour) as the SSR source. Resize-tracked.
- **`map.rs` frame restructure** (water-mode only; normal path unchanged):
  opaque pass (sky/floor/terrain/vector) → resolve colour to `hdr_resolve` +
  write linearized eye-depth to `linear_depth` (a tiny depth-resolve pass,
  `render/depth_resolve.rs`) → **water pass** (own pass, reads `hdr_resolve` +
  `linear_depth`, depth = MSAA depth read-only) → post reads the water-composited
  target. Behind the flag; off = current single-pass water.
- **Subdivided water grid** (`tessellate.rs`): after the lyon fill of a `water`
  polygon, refine into a uniform grid clipped to the polygon (~grid step set so a
  tile carries ≤ N verts; LOD drops the step with tile screen-size). Emits a
  denser `water_mesh` with real interior vertices to displace. Gated by the flag
  (normal path keeps the cheap fill).

**P0 device check:** flag on/off flips cleanly; V1–V5 still render the *current*
look (no regression) through the new pass; trace fps unchanged. (No visual change
yet — this is plumbing.)

---

## Phase 1 — Gerstner vertex displacement (real 3-D waves)

Canonical math — GPU Gems Ch.1, Eq. 9–12
([NVIDIA](https://developer.nvidia.com/gpugems/gpugems/part-i-natural-effects/chapter-1-effective-water-simulation-physical-models)):

Per wave i (direction `Dᵢ` unit, freq `wᵢ=2π/Lᵢ`, amp `Aᵢ`, speed `φᵢ=√(g·wᵢ)`,
steepness `Qᵢ`), with `θᵢ = wᵢ·(Dᵢ·xz) + φᵢ·t`:

```
P.xz += Σ Qᵢ·Aᵢ·Dᵢ·cos(θᵢ)          // horizontal pinch (trochoidal crest)
P.y  += Σ Aᵢ·sin(θᵢ)                 // vertical
```
Steepness clamp (no loops): `Σ Qᵢ·wᵢ·Aᵢ ≤ 1` → use `Qᵢ = Q/(wᵢ·Aᵢ·N)`, `Q∈[0,1]`.
Analytic normal (Eq.10–12, NOT finite differences) with `WA=wᵢ·Aᵢ`, `S=sin θᵢ`,
`C=cos θᵢ`:
```
B = ( 1 - Σ Qᵢ·WA·Dᵢ.x²·S ,  -Σ Qᵢ·WA·Dᵢ.x·Dᵢ.y·S ,  Σ WA·Dᵢ.x·C )
T = (    -Σ Qᵢ·WA·Dᵢ.x·Dᵢ.y·S , 1 - Σ Qᵢ·WA·Dᵢ.y²·S ,  Σ WA·Dᵢ.y·C )
N = normalize(cross(B, T))
```
**Foam factor** = Jacobian of the horizontal displacement (`J<0` ⇒ crest pinch ⇒
whitecap), computed from the same `Σ Qᵢ·WA·…·S` terms — free, reuses the normal sums.

- **Files**: `render/water.rs` (`WaterGlobals` += per-wave params array — 4 waves
  Phase 1), `water_shader.wgsl` vertex stage does the displacement + analytic
  normal + foam on the subdivided grid; the existing domain-warped exp-sin becomes
  a small high-freq **detail** normal added on top (close-up only).
- **Waves from forecast**: 4 waves, wavelengths clustered ½–2× a median that
  scales with `WaterConditions.wave_amp`; directions within ±15° of the MET wind/
  wave bearing; amplitude from sea-state. (`WaterConditions::from_forecast`
  already gives direction + ferocity.)
- **LOD**: wave count + grid density scale with the analytic distance LOD (NOT
  `fwidth`). Far → 1 big swell on a coarse grid; close → 4 waves on the fine grid.

**P1 device check:** V1 must show 3-D wave crests with silhouette against the sky
(parallax as you orbit) + glitter on crests; V3 must stay calm/clean; trace fps ≥
floor. Local golden V1/V3 eyeballed first.

---

## Phase 2 — SSR reflection + sky/cloud probe

SSR per [Casual Effects DDA](http://casual-effects.blogspot.com/2014/08/screen-space-ray-tracing.html)
+ [FidelityFX SSSR](https://gpuopen.com/fidelityfx-sssr/) glossy/mip ideas:

- Reflect view ray about the Gerstner normal; **DDA march in screen space**
  against `linear_depth` (16–32 steps + binary refine). On hit → sample
  `hdr_resolve` (the terrain/mountains). **Mip-blur the sample by roughness +
  ray distance** (cone approximation) — this is what kills the moiré we hit with
  the raw heightfield march. **Edge-fade** the reflection toward screen borders.
- **Miss / off-screen / ray to sky** → sample a **sky+cloud probe**: render the
  analytic sky + the cloud overlay into a small lat-long texture once per N frames
  (`render/sky_probe.rs`). Gives the reflection real *content* (clouds, sun),
  fixing the "plain blue gradient reflection" problem.
- Fresnel blends body (refraction/absorption from P3) ↔ reflection. Keep the
  heightfield march only as the far-distance fallback when SSR rays exit screen.

- **Files**: `render/water.rs` binds `hdr_resolve` + `linear_depth` + sky-probe;
  `water_shader.wgsl` SSR march + probe sample; `render/sky_probe.rs` (new).

**P2 device check:** V2 — the mountains visibly mirror on the water with soft
rippled edges (no moiré at a 1:1 crop); V1 — sky/clouds + sun reflect and shimmer.
Compare device vs golden V2 side by side. fps ≥ floor (SSR is the cost risk —
measure here, drop step count if needed).

---

## Phase 3 — Refraction + depth absorption + foam

- **Refraction**: offset the `hdr_resolve` sample by the normal (xy) → see the
  lake bed / shoreline through the water, Snell-ish.
- **Depth absorption** (Beer's law): extinction by water depth (from `linear_depth`
  minus the surface depth) → shallow = clear/turquoise, deep = dark blue/green.
  Gives the shoreline gradient.
- **Foam**: shoreline (depth-edge from `linear_depth`) + crest (Gerstner Jacobian
  from P1) + a scrolling foam texture/noise; lit by sun.
- Retune SSS, glitter, colour against the now-real reflection.

**P3 device check:** V5 — clear shallows fading to deep offshore + a live foam band
at the shore + foam on breaking crests; V4 — a mountain lake reads correctly
(reflective, depth-coloured) at its elevation.

---

## Phase 4 — LOD, mobile perf, ship

- Tie grid density, wave count, SSR steps, probe refresh all to the analytic
  distance/zoom LOD; verify the full zoom sweep (V1→V3) has no popping and stays
  ≥ fps floor.
- Perf pass on the Pixel: budget the water pass (target ≤ ~6 ms gpu_ms in water
  mode); the SSR + extra resolve are the cost — half-res SSR + reuse if needed.
- Flip `waterMode` default behaviour: toggle stays, but the path is shippable.
- Full V1–V5 device acceptance + golden parity green; PERF logged.

**P5 (optional) FFT/Tessendorf** — only if Gerstner+detail isn't enough; new
compute infra (`render/ocean_fft.rs`, compute pipelines — the engine has none
today). Same V1–V5 acceptance.

---

## Per-phase definition of done (the discipline)

1. Local golden previews (V1–V5 equivalents) rendered and **eyeballed** — no
   regression, the phase's effect visible.
2. Built, installed, device captured at V1–V5 via `tools/water_shots.sh`.
3. Each relevant view meets its written pass criterion; `metrics.txt` shows fps ≥
   floor + gpu_ms within budget.
4. The two GPU goldens (`water`, `water-terrain`) stay green.
5. Only then: commit (with the device shots referenced).

No `fwidth`-based LOD anywhere (driver-divergent — proven this session). Analytic
distance/zoom LOD only.
