# The visual iteration loop (agent-driven)

_2026-06-10_

## Why

Cartographic quality — "does it look like a map?" — is not captured by
pass/fail tests. Bugs here are *visual*: a halo that reads as a box, text
that looks soft, a hillside that turns to noise. The agent can already
**see** rendered PNGs, so the bottleneck was never perception; it was
having the right images, cheaply and repeatably, plus numbers to catch
what the eye misses and to stop regressions.

This is the standing loop for diagnosing and fixing visual issues without
a human as the eyes.

## The loop

```
render → crop → measure → read → diagnose → fix → re-render
```

One command drives the first four steps:

```
cargo run -p turbomap-engine --example visual_lab            # full frame + crops + metrics
cargo run -p turbomap-engine --example visual_lab -- --probe "Kaigaten"   # isolated text
```

It renders the real-data Bergen basemap (the same `turbomap_golden::omt`
scene the `omt-real-bergen` golden pins, so the tool and the credibility
test can never drift) and writes to `--out` (default `/tmp/visual-lab`):

| artifact | what it isolates |
|---|---|
| `full.png` | the whole frame at the chosen `--ratio` |
| `crop-centre.png` | dense centre at **native 1:1** — no display downscaling hiding softness |
| `crop-detail.png` | a hillside/edge region for **detail noise** |
| `probe-text.png` / `-dark.png` | labels **alone** on flat light/dark bands, magnified 3× — glyph edges, halos, sharpness, with nothing else to blame |

and prints a JSON line of metrics:

- **sharpness** — mean acutance (luma-gradient magnitude) over edge
  pixels. Higher = crisper. ~50+ is sharp text.
- **halo_ring** — fraction of edges that ring (dark→light→dark within a
  few px) rather than stepping cleanly. High = bordered/haloed glyphs.
- **speckle_per_k** — isolated high-contrast pixels per 1000. High =
  small-detail noise (a web of thin lines, scattered footprints).

### The key technique: isolation

The decisive move is the `--probe` mode. The first time it ran on dark
background it was instantly obvious that each glyph had a **rectangular
box** around it — invisible in the busy full frame, unmistakable on a flat
field at 3×. That isolation turned a vague "text looks rough" into a
one-line root cause (the glyph atlas was cleared to `0`, which the SDF
shader reads as *inside a glyph*, so every glyph cell's edge ramped 255→0
under bilinear sampling and crossed the fill/halo thresholds). Always
isolate the offending element on a neutral field before theorising.

## Worked example (this is how the boxes/noise were fixed)

1. User: "text glyphs look like they have borders … smaller details look
   like noise."
2. `visual_lab --probe` → read `probe-text-dark.png` → **box around every
   glyph** + `halo_ring` high.
3. Root cause in `text.rs`: `bitmap: vec![0; …]`. Fix: clear to `255`
   ("fully outside"). Regression test `atlas_gutter_is_outside_value`.
4. Re-probe → boxes gone, `sharpness` 119→152.
5. `visual_lab` (full) → `crop-detail.png` → hillside **trail web** =
   `speckle`. Fix in `omt.rs`: drop `service` roads and `path`/`track`
   from the city-zoom style.
6. Re-render → calm hillside; re-pin `omt-real-bergen`.

## Guardrails

- The metrics are **diagnostic**, not asserted in CI — they guide the eye
  and flag regressions during iteration. The committed `omt-real-bergen`
  golden is the CI gate.
- Re-pin the golden (`UPDATE_GOLDEN=1`) only after **looking** at the new
  frame and confirming the change is an improvement.
- Keep the lab scene and the golden scene the same (`golden::omt`) so a
  fix proven in the lab is exactly what the golden locks.
