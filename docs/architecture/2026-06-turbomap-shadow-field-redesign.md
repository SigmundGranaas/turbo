# Turbomap cast-shadow field — high-zoom redesign (scoping)

Status: scoping (2026-06-23). The cast-shadow / AO / water-reflection heightfield
works at mid zoom but produces **no shadows at high zoom (z14+)** — the zooms
used for 3D hiking. This scopes the fix; it is NOT yet implemented.

## The problem, measured

The `TURBO_SHADOW_DEBUG` probe (committed) reports each assembled field's relief
span and the finest resident DEM zoom. On the Sjunkhatten scene, pitch 55°, sun
altitude 18°:

| camera | field extent | relief span (world-z) | DEM finest z | cast-shadow proof |
|---|---|---|---|---|
| z13 | ~30 km | ~1355 m | 13 | **12.4–13k px darkened** ✓ |
| z16 | ~3.8 km | ~177 m | 14 | **0 px** ✗ |

This **rules out** the two obvious culprits:
- NOT a march bug — z13 casts shadows with the identical shader.
- NOT DEM over-zoom — z16's field has a fine DEM (z14 resident) and 177 m of
  real relief across 11,910/65,536 live cells.

**Root cause:** the field's extent is `2.5× the flat footprint`, which shrinks
∝ 1/2^zoom. At z16 the field spans only ~3.8 km, and the per-fragment march
reaches ~¼ of that (`SHADOW_STEPS=64` × `world_size/256` ≈ 947 m). At sun
altitude 18° the sun-ray climbs ~tan(18°)·947 ≈ 300 m over that reach, while the
field holds only 177 m of relief within it — **so the ray clears all terrain and
nothing casts. The occluding peak is simply outside the small high-zoom field.**
At z13 the 30 km field contains 1000 m+ peaks within reach → shadows.

The same fixed-extent field feeds **AO and water reflections**, so they thin out
at high zoom too.

## The fundamental tension

One fixed `HEIGHT_DIM²` (256²) grid cannot be both:
- **Large-extent** — reach distant occluders so a low sun still casts (needs
  many km of coverage at z16), and
- **Fine-resolution** — crisp near shadows (needs metre-scale cells).

Growing the extent (tried: pitch-aware gain, reverted — it didn't restore z16
shadows at the pow2 buckets reached, and trades sharpness) coarsens the near
field; keeping it small loses the occluder. This is the classic shadow-map
extent-vs-resolution problem.

## Hard constraints (must preserve)

- The shipped perf wins stay: progressive (amortized) reassembly, the pan-defer
  (no reassembly while moving), the capped DEM zoom-scan, and `desired ≤ cache`.
- World-locked field (shadows welded to the ground through a pan).
- One field still feeds shadow + AO + water (don't triple the cost).
- Mobile-GPU budget — the device already spends ~170 ms/frame on the 3D passes
  (separate issue); a redesign must not make per-frame GPU worse.

## Options

**A — Minimum-extent floor (smallest change).** Stop the extent shrinking past a
tuned floor (e.g. never below ~8–16 km) so high zoom still covers occluders.
Cost: coarser/softer shadows when zoomed in (the resolution half of the
trade-off), but shadows *exist*. One knob; reuses everything. Risk: at extreme
zoom the 256² over a large floor is blocky — may need the floor zoom-dependent,
or paired with a higher `HEIGHT_DIM` at high zoom (more reassembly cost, which
the progressive build + cap absorb).

**B — Two cascades (near-fine + far-coarse).** A small sharp field for near
shadows + a large coarse field for distant occluders; the shader samples near
where covered, else far. Solves both ends; mirrors cascaded shadow maps. Cost:
a second heightfield (assembly + upload + bind group — note `max_bind_groups=4`
is already tight; water reuses group-3) and a shader cascade-select. Medium-large
effort. (`MEMORY` notes a "cascaded LOD shadows (near+far)" item was once done —
investigate whether remnants exist before rebuilding.)

**C — Sun-biased extent.** Shadows arrive *from* the sun, so bias the field's
coverage toward the sun azimuth (asymmetric extent) — reach the relevant
occluder without paying for a symmetric large field. Medium effort; interacts
with the lattice-snap (the field would re-key on sun movement, already a key
input).

**D — Accept + scope down.** Self-shadowing genuinely diminishes when zoomed into
a small gentle patch; only the mid-zoom band (z14–15, common for hiking) is worth
fixing. Apply A's floor tuned for z14–15 and accept that extreme zoom (z17+) has
faint shadows.

## Recommendation

Start with **A (min-extent floor)**, tuned for the z14–15 hiking band, and
**validate the sharpness trade-off on device** before committing the look. If
A's coarseness is unacceptable at the zooms that matter, escalate to **B
(cascade)**. Do NOT ship a headless guess — the look is the whole point and the
desktop harness can't judge it (Metal renders the passes in ~0.1 ms).

## Validation plan

- `TURBO_SHADOW_DEBUG` relief-span probe (committed) — confirm the field covers
  enough relief at z14–z17 after the change.
- Extend the harness cast-shadow proof to assert darkening at z14 and z16 (it
  currently passes only at z13) — the headless regression gate for "shadows
  exist", though not for sharpness.
- On-device eyeball at z14/z15/z16 with the sun-mode slider — the sharpness +
  coverage judgement that gates the final knob values.

## Effort / risk

| Option | Effort | Risk | Device-gated? |
|---|---|---|---|
| A min-extent floor | small (1–2 knobs + retune) | low (perf), medium (look) | yes (sharpness) |
| B two cascades | medium–large | medium (bind-group pressure, 2× assembly, perf) | yes |
| C sun-biased | medium | medium (re-key on sun move) | yes |
| D accept + A-for-mid | small | low | yes |
