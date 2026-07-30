# On-device routing — UX plan

The engine runs on the phone now (`:core:routing-android`,
`turbo-route-ffi`). This is how a person meets it.

## The principle

**Downloading a map means you can route in it.** Not a mode, not a
setting, not a second thing to manage — a property of an area the user
already downloaded, in an app whose users already understand downloaded
areas.

Everything below follows from that, and the two temptations it rules out
are worth naming because both are the obvious first idea:

- **A "route offline" toggle in Settings.** This is the developer's
  model. A hiker does not want to choose a solver; they want a route in
  a valley with no signal. A toggle also makes the failure the user's
  fault — they left it off — for a decision they had no basis to make.
- **A separate "routing data" download screen.** Two lists of regions
  that can disagree is a support burden and a UI that has to explain
  itself. One region, one download, one size.

## What actually changes for the user

Almost nothing, on purpose. Three things do:

1. Downloading an area gets slightly bigger and gains one line of copy.
2. Routes work with no signal, inside downloaded areas.
3. Off-route reroutes work with no signal — which is the real payoff,
   because that is the moment a hiker is furthest from a cell tower and
   least able to do anything about it.

## Journey 1 — downloading an area

**Where:** the existing `DownloadAreaDialog`, reached from the layers
sheet's "Download this area".

**Change:** routing data is included, always, and the dialog says so.

```
  Download this area?

  ┌──────────────┬──────────────┐
  │   Standard   │   Detailed   │
  └──────────────┴──────────────┘

  About 8.4 MB · 214 tiles will be saved for offline use.
  Includes trail routing, so you can plan routes here without signal.
```

**Why included rather than a checkbox.** The pack is small next to the
tiles it rides with — measured at 2.6 MB for 12.9 × 13.2 km, against
roughly 6 MB of raster at Standard depth for the same area. A checkbox
would ask the user to trade a fifth of the download for a capability they
cannot evaluate in the abstract, and most would leave it at whatever the
default was. Pick the default that makes the feature exist.

**The estimate must include it.** `OfflineEstimate` currently counts
tiles and bytes. It needs the pack's bytes too, or the dialog under-
promises and the `withinLimits` guard is computed against the wrong
number. Pack size is close to linear in area (~16–21 KB/km² measured
across two packs), so an estimate is a multiply, not a server round-trip.

**What if the routing pack fails but the tiles succeed?** The region is
`Complete` for browsing and cannot route. That is a real state and it
needs to be visible on the region card rather than discovered at the
moment someone needs a route — see Journey 5.

## Journey 2 — planning a route with no signal

**The whole design goal: nothing happens.** Waypoints, presets, the
solving card, the result — identical. No badge, no "offline mode"
banner, no toast. A route is a route.

The one honest degradation: **the solving line does not animate.** The
server streams best-path snapshots and the map redraws them; the façade's
`plan` is a single blocking call. The app already seeds
`Solving(Waypoints.roundTrip(...))` — a straight line through the
waypoints — before the first snapshot arrives, so on-device solving shows
that seed and a spinner, then the result. It reads as a slightly slower
solve rather than as a broken one.

Do **not** fake the animation by interpolating toward the seed. It would
be a lie about what the solver is doing, and when streaming does land the
real one would look worse than the fake.

**Expected feel** (estimated from desktop measurement; a phone core is
2–4× slower and this is unverified on hardware): trail routes ~0.5–1.2 s
regardless of distance, because the unified lane is flat in distance.
Cross-country is not flat and is bounded at 10 km by `maxOffTrailKm`.

## Journey 3 — the edge of coverage

The hard case, and the one that decides whether this feels solid or
flaky. A user pans past the edge of what they downloaded and taps.

**Gate before the tap commits, not after the route fails.** `hasCoverage`
is one DEM lookup. On waypoint placement, if the point is outside every
pack *and* there is no connectivity, refuse the placement with a reason
and an action:

```
  ┌────────────────────────────────────────────┐
  │  You haven't downloaded this area          │
  │  and you're offline.                       │
  │                            [ Download ]    │
  └────────────────────────────────────────────┘
```

**"Download" opens the download dialog pre-filled with the bbox that
covers the route**, not the current viewport. The user's intent is a
route, not a rectangle. `RouteCorridor.bounds(geometry)` already computes
this for `downloadAlongRoute`; the same call works on the raw waypoints
before a route exists.

There is a chicken-and-egg here worth being explicit about: you cannot
download "along the route" before you have a route, which is exactly why
packs are region-shaped rather than corridor-shaped, and why the pre-fill
uses the waypoint bbox.

**Straddling two packs.** `PackStore.covering` requires one pack to
contain *every* waypoint, and returns null otherwise rather than picking
the nearest. A route that half-fits fails mid-solve with an error about
terrain, which reads as "the router is broken" instead of "you have not
downloaded that area". Two adjacent regions that a route crosses is a
real scenario for a user who downloaded two valleys, and the honest
answer today is a prompt to download the span as one region. Merging
packs at runtime is possible but is engine work, not UX work.

## Journey 4 — following, and going off-route

The payoff, and the least visible part of the design.

`RouteViewModel.reroute` fires when the user drifts off the line while
following. Today that is a network request from somewhere with, by
definition, poor signal. On-device it is a local solve.

**It must stay silent.** The existing code already goes to some trouble
here — `silentFollow` keeps `Following(oldPlan)` on screen through a
re-solve so the line never flickers, and a failed reroute keeps the old
route rather than throwing an error card at someone who is walking.
On-device routing changes nothing about that contract; it just makes the
reroute succeed where it used to fail.

This is the strongest argument for the whole feature and it should be the
line in the release notes: **your route re-plans itself even where your
phone has no signal.**

## Journey 5 — managing storage

**The region card gains one line, and only when something is wrong.**

```
  Sjunkhatten
  Downloaded · 8.4 MB
```

unchanged when routing works. When the pack is missing or unreadable:

```
  Sjunkhatten
  Downloaded · 5.8 MB
  ⚠ Routing data missing        [ Add ]
```

**Why not always show "routes offline ✓".** A checkmark on every card is
noise that teaches nothing — the user cannot act on it and it is true
everywhere. Surface the exception, not the rule. This also covers the
legacy case: regions downloaded before this feature existed get the
"Add" affordance and cost one tap.

**Deletion deletes both.** One region, one delete, one confirmation. The
existing undo snackbar covers it unchanged.

## When does the phone route, and when does the server?

The decision that most affects perceived quality, and the one worth
staging rather than getting right in one shot.

### Release 1 — fallback only

**Server first. Device when the server cannot answer.**

- No connectivity → device (if a pack covers it)
- Server request fails or times out → device (if a pack covers it)
- Otherwise → server, exactly as today

**Why start here:** it is strictly additive. Nobody who is happy today
gets a worse experience, nobody loses the progress animation on the happy
path, and the on-device path earns trust on the cases where the
alternative was *nothing at all*. If the device path is wrong in some way
we have not found, it is wrong only where the user currently gets a
failure.

The timeout matters more than it looks: a validated-but-dead connection
is the worst case in the mountains, and a route request that hangs for 30
seconds is worse than one that fails fast. Suggested budget ~4 s before
falling through, measured against real server latency rather than
guessed.

### Release 2 — device first, once streaming lands

**Device when a pack covers the route, server otherwise.**

Predictable, no radio, no data, works when connectivity lies. This is the
better end state, but it should not ship before:

1. Progress events cross the FFI, so the animation does not regress; and
2. Latency is measured on real hardware, not extrapolated from a desktop.

Until both hold, release 1's ordering is the honest one.

### Not recommended: a race

Starting both and taking the first to answer is tempting and does give
the best latency. It also doubles the work per route, burns the radio
that release 2 exists to avoid, and makes "which engine answered?"
non-deterministic — which turns any geometry difference between server
and device into an intermittent bug report. Revisit only if measurement
shows the device path is too slow to lead with.

## Failure states, and what the user does about each

The error variants exist because the user's next action differs. The
mapping is already in `OnDeviceRouteRepository.message`; this is the UX
contract it implements.

| Engine error | What the card says | Action offered |
|---|---|---|
| `OutsideCoverage` | That's outside the map you've downloaded. | **Download this area** (pre-filled) |
| `EndpointBlocked` | One of your points is somewhere you can't walk — water, maybe. | Nothing; the user drags the pin |
| `TooLong` | That's too far for one route. Try a shorter leg. | Nothing; the user adds a stop |
| `NoRoute` | No route through this terrain. | Nothing — an honest answer |
| `Pack` | The downloaded map couldn't be opened. | **Download again** |
| `InvalidRequest` | (should be unreachable) | — |

Two rules for this table:

- **Never collapse these into "couldn't find a route".** Doing so is
  precisely what makes an app feel broken, because the user has no way to
  tell a fixable problem from an impossible one.
- **`NoRoute` gets no action, and that is correct.** A fallback straight
  line would be, in this codebase's own words, semantically a lie.

## Copy

Every new string needs an `nb` translation in the same commit — the app
ships bilingual and a half-translated dialog is worse than an English
one, because it reads as a bug rather than a gap.

| Key | en | nb |
|---|---|---|
| `offline_download_includes_routing` | Includes trail routing, so you can plan routes here without signal. | Inkluderer ruteplanlegging, så du kan planlegge turer her uten dekning. |
| `offline_routing_missing` | Routing data missing | Rutedata mangler |
| `offline_routing_add` | Add | Legg til |
| `route_outside_downloaded` | You haven't downloaded this area and you're offline. | Du har ikke lastet ned dette området, og du er offline. |
| `route_download_this_area` | Download | Last ned |

Tone check against the existing strings: the app says "Open the layers
sheet on the map and tap 'Download this area'" — plain, second person,
no jargon, no exclamation marks. None of the new copy says "cache",
"pack", "engine", or "on-device".

## How we would know it works

Instrumentation, because "it feels fine on my phone in a city" is not
evidence about a valley.

- **Which engine answered, and how long it took.** Per solve: source
  (server/device), lane, distance bucket, duration, outcome. The single
  most useful number is device p95 by distance bucket — it is what
  decides whether release 2 is safe.
- **Fallback rate.** How often the server times out and the device
  catches it. If this is near zero, release 1 bought nothing and release
  2 is the whole feature. If it is high, release 1 was the right call.
- **Coverage misses.** How often a route is attempted outside every pack.
  A high number means the download flow is not steering people to the
  right regions, which is a UX bug, not an engine one.
- **Geometry divergence.** When both paths run, do they agree? The
  requirement is equivalence, not bit-identity, so the metric is route
  length and Fréchet distance, not a hash. A divergence that users would
  notice is a calibration drift between the app build and the server.

## Sequencing

1. **Estimate + download includes the pack.** Nothing user-visible works
   until packs exist on disk. `OfflineEstimate` gains pack bytes;
   `DownloadSpec` gains the pack; the service fetches it. Requires the
   server-side pack endpoint, which does not exist yet — that is the
   dependency to schedule first.
2. **Hilt binding with the release-1 policy**, plus the timeout. Small,
   and reversible with one line.
3. **Coverage gating on waypoint placement**, with the pre-filled
   download prompt. The highest-value UX work in the list, because it
   converts a confusing failure into an offer.
4. **Region card exception line + "Add"**, covering legacy regions.
5. **Instrumentation**, before release 2 rather than after, since it is
   what release 2's decision rests on.
6. **Progress events across the FFI**, then release 2's policy flip.

## Open questions

- **Does the pack ride with the tile download, or is it a separate
  fetch?** One region/one download is the UX commitment; whether that is
  one request is an implementation choice with a real failure-mode
  consequence (partial success), and it interacts with the pack endpoint
  that has not been designed yet.
- **Two adjacent regions, one route across the seam.** Prompting to
  re-download the span as one region is honest but wasteful. Multi-pack
  routing is engine work; worth doing only if the metric from "coverage
  misses" says people actually hit it.
- **Is there a real user need for a manual "offline routing" preference?**
  I do not think so, and the plan above deliberately has none. If support
  requests show otherwise, the least-bad version is a data-saver-shaped
  preference ("Plan routes on this phone when possible") rather than an
  engine switch.
