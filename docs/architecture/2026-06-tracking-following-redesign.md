# Tracking, Tracing & Following — Redesign Spec (Android + iOS)

Status: **User stories agreed; product decisions D1/D4/D6/D7 LOCKED
(2026-06-18).** Technical defaults D2/D3/D5 accepted as proposed unless tuned
during build. This is the behavioural spec; the implementation plan is the next
deliverable.

## Why

User feedback (paraphrased): "Following a route and recording a track are
basically the same activity, but the apps treat them as unrelated. Following
should record exactly what I walked, with the same stats. Progress falls apart on
loops/out-and-back. GPS teleports on resume. The map fights me when I pan. Moving
track points is buggy. The live sheet only drags from some spots."

The investigation confirmed every point, on **both** platforms:

- **Following records nothing.** `FollowController` (iOS + Android) only projects
  the latest position onto the planned route and computes progress. The path you
  actually walked is never captured or saved.
- **Progress is global-nearest-point.** `GeoMetrics.progress` finds the closest
  point on the *entire* polyline. On an out-and-back (start ≈ end) it snaps to the
  start when you return → reports ~0 % remaining / "Arrived" while you're still
  out, and tells you you're moving *away* from the goal on the way back.
- **No GPS hygiene.** Neither platform filters by accuracy/staleness/jump (Android
  has a lone 50 m accuracy gate, inside recording only). A stale fix on resume
  teleports the dot, then it snaps back a few frames later.
- **Camera fights manual pan.** Follow mode recenters on every fix and there's no
  gesture detection to release it.
- **Pause semantics diverge.** Android buffers points while paused; iOS silently
  drops them. Neither handles "I forgot to unpause and kept walking."
- **Waypoint move is fragile.** iOS has a snap-back guard but the interaction is
  still unreliable past the first point; Android re-solves on a 300 ms debounce.
- **Live sheet drag is partial.** Only some regions of the sheet respond to drag.

## Glossary (shared model)

- **Session** — one continuous capture of *the path you actually walked* (points,
  distance, moving time, ascent/descent, speed, pace). This is the single source
  of truth. Recording and Following are two **modes of the same session**.
- **Planned route / guide** — an optional polyline you are following (a solved
  route, or a saved track replayed). Present only in Follow mode.
- **Travelled track** — the breadcrumb the Session records. Always captured.
- **Phase / checkpoint** — a sub-point of the planned route (a waypoint/stop, and
  optionally a nearby saved marker). Crossing one records a split.
- **Cursor (`s`)** — your monotonic position *along the planned route*, measured as
  arc-length from 0 → total. The basis for all progress (replaces nearest-point).

---

## US-1 — Following a route also records my track (one engine)

> As a hiker, when I follow a planned route I want the app to record exactly what
> I walked — same points, distance, time, ascent, speed, pace — so that following
> produces a saveable track identical in quality to a plain recording.

**Behaviour**
- Starting a Follow starts a **Session** in Follow mode. The Session records the
  travelled track and *all* the same stats a Recording does. There is no second,
  weaker code path — Record and Follow share one capture engine.
- The only differences in Follow mode are additive: a guide is drawn, progress is
  computed against it, the covered part is dimmed, and phases are marked (US-2/3).
- Pause/resume, background capture, GPS filtering, and auto-resume (US-4/5) behave
  identically to a recording.
- **Always-visible accumulated stats (both modes).** While recording *or*
  following, the live sheet always shows, as the primary readout: **accumulated
  distance**, **elevation gain (ascent)** and **elevation loss (descent)**, plus
  moving time. These are never hidden behind an expand/detent — they are the point
  of the screen. (User: "recording always needs to track and show my accumulated
  distance and height gains and losses.")
- Ending a Follow **auto-saves** the travelled track (D1 — LOCKED), exactly like a
  recording but with no save/discard prompt. The saved track keeps a reference to
  the planned route it followed and the phase splits. Guard: only auto-save if the
  track has meaningful content (proposed ≥ 50 m / ≥ a few points) so an accidental
  tap doesn't litter the list; trivial sessions are dropped silently. Saved tracks
  remain deletable from the list afterwards.

**Feel**
- Following feels like "recording, with a guide on top." Stats never differ
  between the two modes for the same walk.

**Acceptance**
- Follow for 1 km → stop → the saved track's distance/time/ascent equal what a
  plain recording of the same walk would show (±GPS noise).
- The travelled track (your real path) is distinct from the planned route on the
  map and in the saved artifact.

---

## US-2 — Progress that works on loops and out-and-back

> As a hiker doing an up-and-back or a loop, I want progress to reflect how far
> along the route I actually am, so that returning toward the start reads as
> *nearing the finish*, never as "Arrived" or "moving away."

**Behaviour — the cursor model (replaces global nearest-point)**
- Precompute cumulative arc-length for the planned route: each vertex has an `s`
  from 0 to `total`.
- Keep a monotonic **cursor `s_cursor`** (starts 0). On each accepted fix:
  - Find the nearest route point **within a forward window** `[s_cursor − BACK,
    s_cursor + AHEAD]` (proposed BACK ≈ 60 m, AHEAD ≈ 400 m), *not* the global
    nearest. This is what fixes loops: once `s_cursor` is near `total`, a fix near
    the start can't match `s ≈ 0` because it's outside the window.
  - `s_cursor = max(s_cursor, s_match)` (monotonic; small genuine backtracks
    allowed inside the window).
  - **fraction** = `s_cursor / total`; **distance remaining** = `total − s_cursor`
    measured *along the route*; ETA from remaining distance + remaining ascent.
  - **off-route** = perpendicular distance from the fix to the matched route point
    exceeds a threshold (proposed 50 m) for N consecutive fixes.
  - **Arrived** requires *both* `s_cursor ≥ total − tol` **and** physical proximity
    to the route's actual end point (proposed 30 m). Returning to a start that
    coincides with the end only "arrives" once the cursor has traversed the whole
    arc.
- On an out-and-back A→B→A: climbing advances the cursor toward 50 % at the
  turnaround B; descending matches the return leg (arc > 50 %) inside the forward
  window, advancing toward 100 % at A. No false arrival, no "moving away."

**Progress shows two things at once**
- **Planned progress** — how far along the guide you are (the dimmed-vs-remaining
  split, the % and distance-remaining).
- **Actual track travelled** — your real breadcrumb, drawn distinctly, so you can
  see deviation from the planned line.

**Progress indicator — ONE control, wavy = the tracked part (corrects current
Android)**
- There is a **single** progress indicator for `fraction` — **not two
  sliders/bars**. (The current Android live UI has a duplicate; remove it.)
- The indicator IS the progress tracker: its filled length equals the cursor
  fraction. The **completed/tracked portion renders as a wavy line**; the remaining
  portion is flat — this is the whole point of Material 3 Expressive's
  `LinearWavyProgressIndicator` (Android) / its iOS equivalent. The wave grows as
  you progress; it is not a decorative spinner.
- The wave animates **slowly and calmly**. The current Android wave is "crazy
  fast" — it must be a gentle, near-static undulation that reads as a progress
  readout, not motion for motion's sake. (User: "It needs to be really slow, and
  that should be the progress tracker… render the tracked part as wavy to show the
  progress. There should not be two sliders. The agent that made this completely
  misunderstood my request.")

**Feel**
- The number only ever climbs as you make forward progress. Backtracking a little
  for a wrong turn doesn't yo-yo the percentage; going far off prompts off-route.

**Acceptance**
- Scripted out-and-back fixture: cursor is ~50 % at turnaround, ~100 % at return;
  never reports Arrived before the turnaround; never reports a negative "closing"
  on the return leg. (New test; current code has zero loop coverage.)

**Decisions**
- ❓**D2: window sizes** (BACK/AHEAD) and off-route threshold — proposed values
  above; confirm or tune.

---

## US-3 — Phases / checkpoints and a dimmed travelled guide

> As a hiker, I want the part of the guide I've already covered greyed out and the
> waypoints turned into checkpoints, so I can see at a glance what's left and at
> which point/time I crossed each marker.

**Behaviour**
- The planned route is drawn in two styles split at `s_cursor`: **covered**
  (dimmed/grey) behind you, **remaining** (highlighted) ahead.
- Each **phase** = a planned-route sub-point (waypoint/stop). When `s_cursor`
  passes a phase's arc-length, the phase is marked **crossed** with a timestamp
  and a split (time + distance since the previous phase). Crossed phases render
  filled/checked; upcoming ones are outlined.
- The live sheet lists phases with splits ("Checkpoint 2 · 14:32 · 1.8 km · 22
  min"), and the next phase is highlighted ("Next: Saddle, 600 m").
- ❓**D3: are phases only the route's own waypoints, or also nearby *saved
  markers* the route passes (within e.g. 40 m)?** Proposed: route waypoints by
  default; include passed saved markers as additional checkpoints.

**Feel**
- Like split times on a running watch — you always know the last checkpoint you
  hit and the next one coming.

**Acceptance**
- Crossing a waypoint logs a split with a time; the covered polyline grows behind
  the cursor; the phase flips to "crossed."

---

## US-4 — Pause that survives forgetting to unpause

> As a hiker, if I pause and then forget to unpause, I don't want to lose the walk
> I did while paused — I want the app to keep capturing in the background and let
> me re-apply or discard that segment.

**Behaviour**
- **Pause does not stop GPS.** While paused the Session keeps receiving filtered
  fixes into a **held buffer** (not yet part of the track). Moving time and the
  visible distance freeze.
- If movement during pause exceeds a threshold (proposed: > 80 m cumulative *or*
  sustained motion for > 60 s), the app **proactively nudges**: "Still moving?
  Recording is paused — resume?" (lock-screen/notification + in-app banner).
- On **resume**, if the held buffer contains meaningful movement, ask: **"Include
  the 320 m you moved while paused?"** → *Include* stitches the buffer into the
  track (back-dated); *Discard* drops it and the track continues from here.
- ❓**D4: default behaviour when motion is detected during pause** — (a)
  auto-resume + keep, notify; (b) keep buffering, ask Include/Discard on resume;
  (c) only nudge, change nothing until I act. Proposed: (b) + the proactive nudge.
- Crash/kill safety: the buffer and the in-progress track are persisted (Android
  already has a `RecordingDraftStore`; iOS needs the equivalent) so a forgotten,
  backgrounded session is recoverable.

**Feel**
- You never silently lose distance. The app gently catches "you forgot to unpause"
  and hands you the choice, instead of either dropping the data or padding your
  stats without asking.

**Acceptance**
- Pause → walk 200 m (scripted) → resume → prompt offers Include/Discard; Include
  yields a continuous track through the paused segment, Discard yields a gap.

---

## US-5 — GPS that doesn't teleport or snap to garbage

> As a hiker, I don't want the dot to jump to a wrong place when I reopen the app
> and then snap back, and I don't want inaccurate fixes distorting my track or
> distracting me.

**Behaviour — one shared fix filter (both platforms)**
- **Accuracy gate:** drop fixes with horizontal accuracy worse than a ceiling
  (proposed 50 m, matching Android's recording gate) for *all* consumers (map dot,
  recording, following) — not just recording.
- **Staleness gate:** drop fixes older than ~5 s (CoreLocation/Android can deliver
  a cached fix first on resume — this is the teleport source).
- **Jump gate:** reject a fix that implies an implausible speed vs the previous
  accepted fix (proposed > ~30 m/s for on-foot), unless several consistent fixes
  confirm it (so a real fast move isn't permanently rejected).
- **Resume guard:** on foreground, do **not** move the dot/camera to the first
  fix until it passes the gates *and* is fresh; show the last good position
  meanwhile. No "teleport then snap back."
- Snapping target: the map dot and any camera-follow use the **filtered** stream;
  the recorded track uses the same filtered stream plus the existing ≥3 m step
  gate.

**Feel**
- Reopening the app shows you where you actually are, immediately and stably — no
  flicker to a ghost location.

**Acceptance**
- Inject a stale/low-accuracy fix on resume → the dot does not move to it; the
  recorded distance gains nothing from it.

**Decisions**
- ❓**D5: thresholds** (accuracy 50 m, staleness 5 s, jump 30 m/s) — proposed;
  confirm or tune per how aggressive we want filtering in poor signal.

---

## US-6 — Smart "track-me" camera: auto on open, release on pan

> As a hiker, I want the map to follow me when I open the app, but the instant I
> pan the map myself it should stop following, so I'm never yanked back.

**Behaviour**
- On app open, camera **auto-centres and follows** the (filtered) location.
- A **user-initiated** pan/zoom/rotate **immediately disables** follow (no
  recenter on the next fix). The follow control reflects the off state.
- Tapping the follow control re-enables follow and recenters.
- While **recording or following a track**, follow is on by default but the same
  manual-pan-releases rule applies (so you can inspect the route, then tap to
  re-follow). Camera updates use the filtered stream and are lightly smoothed (no
  jitter from raw fixes).
- Implementation note: distinguish user gestures from programmatic camera moves
  (iOS: gesture recognizers / `regionWillChange` driven by a user-interaction
  flag; Android: map gesture listener) so programmatic recenters don't self-cancel.
- ❓**D6: scope of auto-follow** — (a) every app open; (b) only while
  recording/following; (c) remember last choice. Proposed: (a) on open + always-on
  during record/follow, both released by manual pan.

**Feel**
- The map helps without nagging: it puts you on screen, then gets out of your way
  the moment you take control.

**Acceptance**
- Open app → camera follows. Pan once → following stops and stays stopped. Tap
  follow → recenters and resumes.

---

## US-7 — Reliable relocation of track points

> As someone planning a track, I want to move any waypoint reliably — the 1st, the
> 5th, all of them — without it snapping back or behaving erratically.

**Behaviour**
- **Both** drag and tap-to-pick→tap-to-place ship from day one (D7 — LOCKED), and
  **both must be reliable** for every point on every device — not just the first.
  The current drag is fragile past the first point and barely works in the
  simulator; that must be fixed, not worked around. Tap-to-place is the
  automatically-tested path (drag can't be E2E-driven on the map); drag gets
  on-device verification and the snap-back guard must hold across re-solves and
  rapid sequential edits.
- No snap-back: a point stays where you put it; re-solving the route never resets
  the point you just moved. (iOS has a drag-id guard; the spec requires this hold
  across re-solves and rapid sequential edits, verified on-device, on both
  platforms.)
- Works the same in Route, Line, and Draw track modes where applicable.

**Feel**
- Editing a planned track feels precise and trustworthy — every point obeys.

**Acceptance**
- Move 4 different waypoints in sequence; each lands and holds; the solved line
  follows; nothing snaps back. Covered by an automated map-tap test (reachable via
  the rail entry, since long-press can't be E2E-driven).

---

## US-8 — A live sheet that drags from anywhere

> As a hiker, I want to drag the recording/following sheet up and down from
> anywhere on it, so resizing/dismissing feels consistent.

**Behaviour**
- The entire sheet surface is draggable between its detents (mini / expanded), not
  just a handle or some regions. Inner scroll content and the drag gesture
  coexist (drag the sheet when content is at its scroll top; otherwise scroll).
- Detents and dismissal behave predictably; the map keeps the user dot above the
  sheet inset.

**Feel**
- The sheet feels like one solid, grabbable surface — no dead zones.

**Acceptance**
- Drag initiated from the stats area, the title, and the body all move the sheet.

---

## Cross-cutting architecture (proposed, for the impl phase)

- **One Session engine** owns capture (travelled track + stats + pause/buffer +
  draft persistence) and the **filtered** location stream. **Guidance** (planned
  route + cursor progress + phases) is an optional component attached in Follow
  mode. Collapses today's separate `RecordingController` and `FollowController`
  into `Session` + `Guidance`, shared in `core` and mirrored on both platforms.
- **`RouteProgress`** is rebuilt around the arc-length cursor (US-2), replacing
  `GeoMetrics.progress`'s global-nearest-point. Pure + heavily unit-tested
  (straight, loop, out-and-back, off-route, skip-ahead fixtures).
- **`LocationFilter`** (US-5) sits in front of every consumer, shared core logic.
- Map camera-follow gains user-gesture detection (US-6) in each platform's map
  wrapper (`TurboMapView` / the Android map screen).

## Open decisions to resolve (summary)

| # | Decision | Resolution |
|---|----------|------------|
| D1 | Follow finish → save flow | **LOCKED: auto-save** the travelled track every time (guard: skip trivial sessions); keeps planned-route + splits ref; deletable later |
| D2 | Cursor window / off-route thresholds | Proposed (accepted unless tuned): BACK 60 m, AHEAD 400 m, off-route 50 m × N fixes |
| D3 | Phases source | Proposed (accepted unless tuned): waypoints + passed saved markers (≤40 m) |
| D4 | Motion-during-pause default | **LOCKED: buffer + nudge; ask Include/Discard on resume** |
| D5 | GPS filter thresholds | Proposed (accepted unless tuned): accuracy 50 m, staleness 5 s, jump 30 m/s |
| D6 | Auto-follow scope | **LOCKED: on every open + during record/follow; released by manual pan** |
| D7 | Waypoint move interaction | **LOCKED: both drag and tap-to-place from day one, both reliable** |
