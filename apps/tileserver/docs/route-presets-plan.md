# Proposal + Plan: Trip presets + fixing road-detour behaviour

## The problem, diagnosed

Routes take big detours onto the road network. Root cause (confirmed in
code): the unified solver prices every graph edge through the
`CostContributor` stack via `compose_edge_walk_seconds`, and **there is no
surface contributor in that stack**. The `surface_multiplier` table in
`cost-config.toml` (`foot: vei = 1.6`) is *baked into the graph artifact at
build time* and was only ever read by the old graph-Dijkstra path — the
unified solver (the default since the rework) ignores it.

So today, in the unified solve:
- a car road (`vei`) costs ≈ base pace (same as a trail),
- a marked trail (`sti`) gets only a small `MarkingBonus` (≈ 0.85×),
- off-trail mesh costs `off_trail_base` × base pace (foot = **2.3×**).

A road is therefore ~2.3× cheaper than cutting across open ground, so the
solver will gladly take a road detour up to ~2.3× longer than the direct
line. That's exactly the behaviour you're seeing.

**What you want:** roads only *slightly* better than off-trail; trails still
good; no big detours; height-difference and "extreme directness" tunable —
all via **simple, preconfigured presets**.

## Decisions locked (2026-05-31)
- **Balanced is the global default** — every route gets road-avoidance with
  no preset selected; presets adjust from there. Recalibrate so existing
  trail-following corpus cases don't regress.
- **Road penalty (Balanced): `vei = 2.0` vs `off_trail_base = 2.3`** — a road
  must be < ~1.13× the direct-line distance to win. Slightly-better-than-
  off-trail, not equal.

## Proposal (two parts)

**Part A — make surface cost a real, runtime, tunable contributor.**
Add a `SurfacePaceContributor` that reads `EdgeRecord.fkb_type` and applies a
per-surface, per-profile pace multiplier *in the live solve* (so it affects
the unified router and is overridable per request — no graph rebuild). Default
foot values make car roads only marginally cheaper than open ground:

| surface | today (baked, ignored) | new default (foot) | effect |
|---|---|---|---|
| `sti` (trail) | 1.0 | 1.0 | trails stay the best surface |
| `traktorvei` | 1.4 | 1.4 | farm/forest track, fine |
| `skogsvei` | — | 1.6 | gravel forest road, mild penalty |
| `vei` (car road) | 1.6 | **2.0** | only slightly better than off-trail (2.3) |
| `skiloype` | 1.2 | 1.2 | groomed track |

With `vei = 2.0` vs `off_trail_base = 2.3`, a road must be < ~1.13× longer
than the direct line to win — big detours die immediately. This fixes the
default even before presets, and gives presets their main lever.

**Part B — presets: simple, named trip styles.**
A small TOML registry of named presets, each a bundle of cost knobs with a
friendly label + one-line description. The API exposes the list; a request
takes `preset: "<name>"`; the SPA shows a dropdown ("Trip style") as the
primary control, with the existing advanced sliders demoted to a collapsed
"fine-tune" section that overlays on top of the chosen preset.

## Proposed presets

All values are foot-profile starting points, to be calibrated against the
scenario corpus. Knobs: `off_trail_base` (willingness to leave any
trail/road), `surface_pace.vei` (road avoidance), `trail_proximity` (pull
toward marked trails), `total_gain.amplifier` (climb aversion → "less height
difference"), `grade_limited.max_grade_deg` (steepness cap).

| Preset | off_trail | vei | trail bias | gain amp | max grade | Feel |
|---|---|---|---|---|---|---|
| **Balanced** (default) | 2.3 | 2.0 | normal | 1.0 | 27° | Prefer trails; roads avoided unless they genuinely help; no big detours. |
| **Avoid roads** | 2.5 | 2.6 | strong | 1.0 | 27° | Roads ≈ open ground. Stay in nature; only touch tarmac when unavoidable. |
| **Direct (extreme)** | 1.3 | 1.3 | weak | 0.6 | 35° | As-the-crow-flies; accept steep ground and off-trail; minimal detouring. |
| **Easy grade** (less climbing) | 2.5 | 2.0 | normal | 2.5 | 18° | Flat and gentle; switchbacks over steep climbs; accepts a longer route. |
| **Trail purist** | 4.0 | 3.0 | very strong | 1.2 | 27° | Marked trails almost always; off-trail only to connect. |

Notes:
- "Less height difference" = high `total_gain.amplifier` (climbs cost more)
  + low `max_grade_deg` (forces gentler traverses/switchbacks). Both already
  exist as runtime knobs.
- "Extreme" = low `off_trail_base` + high `max_grade` + low gain amp + weak
  trail pull.
- Presets are data, not code — adding/editing one is a TOML edit + restart.

## Stages

### Stage 1 — `SurfacePaceContributor` (runtime surface cost)
- `native_contributors.rs`: new contributor reading `ctx.kind` →
  `EdgeRecord.fkb_type`, returning a multiplicative `pace_factor` from a
  per-profile `BTreeMap<surface, f32>` (mesh edges → 1.0). Mirrors
  `MarkingBonusContributor`'s shape; uses the existing `pace_factor` channel
  so it composes cleanly in `compose_edge_walk_seconds`.
- `config.rs` + `cost-config.toml`: add `[surface_pace.{foot,bicycle,ski}]`
  (replaces the dead baked `surface_multiplier` as the live source of truth;
  keep the old table or migrate it). Add `CostConfigPatch.surface_pace_*`
  override fields (per surface) so presets/requests can tune them.
- `main.rs`: register the contributor at boot.
- **This stage alone fixes the default detour behaviour** and is verifiable
  on its own.

### Stage 2 — Preset registry + resolution
- `tools/route-presets.toml`: `[[preset]]` entries (`name`, `label`,
  `description`, and the knob fields → a `CostConfigPatch`).
- `config.rs`: load presets at boot into `Vec<Preset { name, label,
  description, patch: CostConfigPatch }>`; embed defaults via `include_str!`.
- Resolution: `preset → patch`, then overlay any explicit
  `cost_config_override` on top (preset = base, sliders = fine-tune).

### Stage 3 — API
- `v1/pathfind.rs`: `PathfindReq` gains `preset: Option<String>`. Resolve to a
  patch server-side, merge with any `prefs.cost_config_override`, set it on
  `Prefs`. Applies to `/pathfind`, `/record`, `/stream`.
- `GET /v1/route/presets` → `[{ name, label, description }]` for the SPA.
- Unknown preset → 400 with the list of valid names.

### Stage 4 — SPA UX
- `PlotRoute.tsx`: a **"Trip style"** dropdown (primary control, top of the
  panel) populated from `/v1/route/presets`, each showing label +
  description. Selecting one sets the active preset on every request.
- Demote the existing cost sliders into a collapsed "Fine-tune (advanced)"
  section that overlays the preset (keeps power-user control without
  cluttering the simple path).
- Persist the last-used preset in `localStorage`.

### Stage 5 — Calibration + validation
- Extend `tools/route-scenarios.toml` with a **road-detour case**: endpoints
  where a road detour currently wins; assert Balanced keeps `vei` metres low
  and length within ~1.15× of the direct line.
- Add a tiny "preset matrix" check: run the same 3–4 query pairs through each
  preset and assert the expected ordering (Direct shortest/steepest; Easy
  grade lowest ascent; Trail purist highest `sti` %; Avoid roads lowest `vei`
  %). Self-validate with the PIL renderer + `terrain_metrics.py`.

## Critical files
| Concern | File |
|---|---|
| Surface pace contributor | `crates/turbo-tiles-pathfind/src/native_contributors.rs` |
| Config + patch fields + preset loader | `crates/turbo-tiles-pathfind/src/config.rs`, `tools/cost-config.toml`, `tools/route-presets.toml` (new) |
| Boot wiring | `crates/turbo-tiles-bin/src/main.rs` |
| API: `preset` field + presets endpoint | `crates/turbo-tiles-api/src/v1/pathfind.rs` (+ router) |
| SPA dropdown + advanced fold | `apps/admin/src/screens/PlotRoute.tsx`, `apps/admin/src/api/v1.ts` |
| Validation | `tools/route-scenarios.toml`, `terrain_metrics.py` |

## Verification gate
```
cargo test -p turbo-tiles-pathfind         # surface contributor unit cost
cargo build --release --bin tileserver
cargo test --workspace --test scenarios    # incl. road-detour + preset matrix
# Visual: same route under each preset via the SPA / shot script
```
Pass criteria: Balanced no longer detours onto roads (road metres ↓, length
within ~1.15× direct); each preset produces its intended character; trails
still preferred; 2-point + multi-waypoint behaviour unchanged otherwise.

## Risks & mitigations
- **Recalibration of the default changes existing routes.** Mitigate: lock
  the current trail-following corpus cases first; tune `vei`/`off_trail_base`
  against them so trail routes don't regress while road detours shrink.
- **Per-surface knobs interact with `off_trail_base`.** They're all pace
  multipliers in one walk-seconds field, so the ordering is predictable
  (road < off-trail iff `surface_pace.vei` < `off_trail_base`); calibrate the
  gap, not absolute values.
- **`skogsvei` / surface coverage in the graph.** Confirm `fkb_type` actually
  distinguishes gravel forest roads from car roads; if not, fold into `vei`
  and note it.
- **Preset sprawl.** Keep to ~5 curated presets; everything else is the
  advanced fold. Presets are data, so adding more later is cheap.

## Analysis addendum (2026-05-31): same-class bugs as the road issue

The road bug was "a cost lever exists in config but the unified solver
silently ignores it." Auditing for the same class turned up more, confirmed
empirically (per-request override → byte-identical route/cost):

1. **Per-request `cost_config_override` is ignored by the unified solver for
   slope, gain, trail-proximity and water.** `solve_unified_path` passes the
   BOOT-built contributors (`contributors_for_breakdown()`), which bake their
   config at startup; it never rebuilds them from the effective (patched)
   config. Only `off_trail_base` (a param) and `surface_pace` (now injected
   with effective config) honour overrides. Proven: `total_gain_amplifier=5`,
   `slope_cell_refuse_above_deg=20`, `trail_proximity_bonus_at_zero=0.01`,
   `water_cost_s_per_m=0` each changed NOTHING on two test routes. The same FMM
   path was fixed earlier (it rebuilds from `effective_cfg`); the unified
   default never was. → **The SPA calibration sliders are also no-ops on the
   default solver today.**
2. **The unified off-trail MESH uses hardcoded slope/grade constants**
   (`MAX_GRADE_DEG=27`, `STEEP_PENALTY_K=10`, `CLIFF_DEG=60`, inlined
   `tobler_pace`) and has NO gain term — it ignores `slope_cell.*` and
   `total_gain.amplifier` entirely. So "less height difference" / steepness
   limits can't shape off-trail ground in the default solver (only the opt-in
   elastica path reads `grade_limited.max_grade_deg`).
3. **The baked graph cost is dead for routing.** `surface_multiplier` and the
   graph's baked per-profile cost arrays are recomputed-from-scratch by the
   unified contributor stack, so they're unused (now superseded by
   `surface_pace`). Harmless but confusing; should be removed or documented.
4. **`MarkingBonusContributor` is hardcoded** (`::default()`), not config- or
   override-driven — a fixed lever with no knob.

### Impact on the presets
- **Work today** (driven by `off_trail_base` + `surface_pace`, which are live):
  Balanced, Avoid roads, and the road/off-trail character of Trail purist &
  Direct.
- **No-ops until fixed**: Easy grade (gain + max-grade), the steepness/gain
  character of Direct, and the trail-proximity pull of Trail purist / Avoid
  roads.

### Fix (prerequisite for the full preset set)
- In `solve_unified_path`, REBUILD the config-driven contributors (Tobler,
  GraphSlope, TrailProximity, TotalGain) from `effective_cfg` per request —
  exactly as `surface_pace` and the FMM path already do. Factor the
  `with_defaults_and_config` stack-builder so it can be re-run with a given
  `CostConfig`; keep the data-driven contributors (water/vector/mask) as-is.
- Teach the unified `mesh_step` to read `slope_cell.quadratic_scale_deg` /
  `refuse_above_deg` and a gain term from `total_gain.amplifier`, instead of
  the inlined hardcoded constants, so off-trail steepness/climb-aversion is
  tunable. (Add `surface_pace`/grade patch fields already done.)
- Add `grade_limited_max_grade_deg` use to the unified mesh, or document that
  hard switchback grade caps remain elastica-only.

This fix is small and high-leverage: it makes the existing SPA sliders AND the
full preset set actually affect the default routes.

## Out of scope
- Per-segment / per-leg presets (one preset per route for now).
- Auto-suggesting a preset from terrain.
- Profile-specific preset sets beyond foot (bicycle/ski can reuse Balanced
  until separately calibrated).
