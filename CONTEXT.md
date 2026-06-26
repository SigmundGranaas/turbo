# Context — Ubiquitous Language

The project glossary. Terms here are domain/UX concepts shared across the
Flutter, Android, iOS and web clients — **devoid of implementation detail**.
Architectural/implementation decisions live in `docs/architecture/`, not here.

## Map Tool

A distinct, self-contained map *capability* the user activates on the home map —
each is a mode/overlay layered on the shared map surface. The current tools:

- **Route** — building and previewing a planned route.
- **Live / Follow** — actively following a route or recording, with live stats.
- **Offline** — selecting and downloading a region for no-network use.
- **Markers** — placing and editing points of interest.
- **Radar** — the weather/precipitation overlay.
- **Sun** — time-of-day / sun-position shading of the terrain.
- **Collection picker** — assigning an entity to a saved collection.

Tools are *independent*: the user engages one without the others needing to know.
When one tool triggers another's behaviour (e.g. starting **Follow** from a
**Route**), the interaction goes through a shared domain capability — never tool
to tool. See `docs/architecture/2026-06-android-architecture-remediation-plan.md`
for how this is enforced on Android.

## Follow

Tracking the user's progress along a target — either a planned **Route** or an
in-progress recording — keeping the map camera on them and surfacing live stats
(distance, next checkpoint, off-route state). "Follow" is one engine whether the
target is a route or a recording; it is not a separate "navigation" concept.
