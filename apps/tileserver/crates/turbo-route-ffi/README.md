# turbo-route-ffi

The on-device routing engine, as Kotlin and Swift see it.

```kotlin
val engine = RouteEngine.open("/data/data/.../packs/sjunkhatten")
if (!engine.hasCoverage(here)) { promptDownload(); return }

// Every field but `mode` defaults from Rust, so a retune of the budgets
// or the preset reaches the app without a Kotlin edit.
val route = engine.plan(listOf(here, there), RouteOptions(mode = TravelMode.FOOT))
map.draw(route.geometry)          // WGS84, ready to plot
label.text = "${route.lengthM / 1000} km, ${route.ascentM} m up"
```

That is the whole surface: open a pack, ask for a route. No terrain
objects, no cost model, no solver set.

## Why it is this coarse

Everything below this crate was built to have clean seams — ports at L1,
adapters at L3, a solver registry, a composition root. **None of it
crosses the boundary**, and that is the point. Fowler's First Law of
Distributed Object Design applies literally here: every seam exposed
across FFI becomes a breaking change for two mobile apps the moment it
moves. The seams exist so the Rust can evolve; the façade exists so the
phone doesn't have to care.

The one place layers are deliberately combined is `RouteEngine::open`,
which does the L5 composition — open artifacts, erase them to ports,
take the calibrated constants from `turbo-profile-no`, hand the engine
values. A host has no business doing that, and no way to get it wrong if
it can't.

## Building a pack

Two ways to say which region, because two callers know different things.
The routing gate has a corpus and needs every hike in it covered:

```sh
tileserver slice-pack \
  --src ~/.data/artifacts --dst ./packs/sjunkhatten \
  --corpus tools/sjunkhatten-ci-corpus.toml --halo-m 1000
```

An app has a viewport and no corpus — the whole point of downloading a
region is to route somewhere nobody has walked yet:

```sh
tileserver slice-pack \
  --src ~/.data/artifacts --dst ./packs/sjunkhatten \
  --bbox 14.95,67.02,15.20,67.12 --halo-m 1000
```

A pack is a directory of `norway.{dem,mask,graph,graph_geom}` plus a
`pack.toml` — the artifacts are the same formats the server reads, so
there is no separate pack reader. Measured: **2.6 MB for 12.9 × 13.2 km**,
7.4% of the source artifacts. Roughly 16–21 KB per km², so a 100 × 100 km
region is ~200 MB and all of Norway is ~6 GB. Regional packs are not a
nicety.

Only the DEM is required. Without a graph, routes are cross-country;
without a mask, water and glaciers are not refused. A missing DEM is
fatal, because routing without terrain would return a straight line —
which the codebase elsewhere calls "semantically a lie".

### The manifest

```toml
[pack]
format_version = 1
frame = "utm33n"
extent = [14.95, 67.02, 15.20, 67.12]   # WGS84, the REQUESTED region
halo_m = 1000.0
```

It answers two questions the artifacts cannot. *Which area is this?* —
answerable from the DEM, but only by mmapping and indexing it, and a host
listing six downloaded regions wants six extents rather than six rasters.
*Can this build read it?* — not answerable at all without a version
marker. Artifact files carry per-kind versions, so a format change is
caught; a change in what the bytes **mean** is not, and the symptom is a
route that is quietly wrong rather than a file that fails to open. On a
server that is a bad deploy, noticed in minutes. On a phone the pack sits
on the user's disk until they delete it.

`extent` is the region that was **asked for**, not the ground the pack
contains — the halo and whole-tile alignment both make the latter larger,
and advertising it would offer routing in a margin whose only job is to
make routing *inside* the region correct.

A pack without a manifest still opens, falling back to the DEM's bounds.
Packs predate it, and the artifacts have always been enough to route.

**Not in the pack: the cost config.** The design sketch put it there so
device and server geometry would match bit for bit. The requirement is
equivalence — the same route, not the same floats — so the calibration
stays with the engine build and the pack stays free of its own tuning.

## Generating the bindings

```sh
cargo build -p turbo-route-ffi
cargo run -p turbo-route-ffi --bin uniffi-bindgen -- \
  generate --library target/debug/libturbo_route_ffi.so \
  --language kotlin --language swift --out-dir target/ffi-bindings
```

For Android, build the cdylib per ABI with `cargo-ndk` and ship it beside
the generated Kotlin:

```sh
cargo ndk -t arm64-v8a -t armeabi-v7a -o ./jniLibs build --release -p turbo-route-ffi
```

## Three things a host must handle

**The two budgets are load-bearing, not advisory.** Writing this crate's
tests found why: a route from Oslo to a Sjunkhatten pack — 850 km, one
endpoint outside coverage entirely — **solved, in 83 seconds**. On a
server that is a slow request. On a phone it is an ANR and a flat
battery.

This README used to say the engine's `max_off_trail_km` bounded the
cross-country mesh and only the trail-network case was unbounded. That
was wrong: the knob was declared, defaulted, hashed into the leg
fingerprint and echoed by `/v1/debug/prefs`, and **read by nothing**.
Nothing was bounded, which is why 850 km solved at all. It is enforced
now, in `FmmGradeLimited::solve`.

They are two budgets because the lanes cost very different amounts.
Measured through this façade against the CI pack, release, on a desktop:

|        | unified | cross-country |
|--------|--------:|--------------:|
| 1 km   |  251 ms |        427 ms |
| 4 km   |  249 ms |        734 ms |
| 6 km   |  293 ms |      1 891 ms |
| 11 km  |  265 ms |      8 515 ms |

The unified lane is flat in distance — adaptive cell sizing bounds its
work. The cross-country lane is not. One budget for both would be either
too tight for trail routing or too loose for terrain, so `maxSpanKm`
(100 km) bounds the request and `maxOffTrailKm` (10 km) bounds the
expensive lane. `RouteError.TooLong` names which one refused.

Opening a pack costs **323 ms** on the same machine, nearly all of it the
trail R-trees. Hold the engine for the app's lifetime.

**`OutsideCoverage` and `EndpointBlocked` are separate for a reason.**
The user's fix differs — download more map, versus move the pin off the
lake — and collapsing them into one "no route" is what makes an app feel
broken. `hasCoverage()` is one DEM lookup, so gate the UI with it rather
than finding out from a failed route.

## Panics do not cross

Every exported method wraps its work in `catch_unwind`. A Rust panic
unwinding into the JVM or the Objective-C runtime is undefined behaviour
and in practice aborts the process — the user's hiking app disappears
because the solver hit an edge case on a ridge. It becomes
`RouteError.Internal` instead. A caught panic is still a bug; it is just
one that leaves a request failed rather than the phone with no app.

## Determinism

E0 and E1 established that the solver is bit-identical across x86_64 and
aarch64, so a route computed on a phone matches the one computed on the
server. `tools/boundary_check.sh` forbids the build flags that would
forfeit that (`target-cpu=native`, `+fma`, fast-math).

Re-verified at this façade, including the `roundTrip`, `avoid` and
cross-country paths: all four geometry hashes are identical between
x86_64 and aarch64, and the 14 host tests pass on both.

**Still open:** that is glibc-vs-glibc under QEMU. Bionic on real silicon
is unverified — run the host tests on a device before relying on
server/device agreement. Note that the requirement is *equivalence*, not
bit-identity, so a divergence here is a quality question rather than a
correctness one.

## The generated Kotlin

`RouteEngine.open(dir)` is a **companion function**, not a constructor — uniffi
maps a named Rust constructor that way, and `RouteEngine(x)` resolves to the
internal pointer constructor instead. Errors arrive as a sealed
`RouteException` with one subclass per variant. `RouteOptions` carries the Rust
defaults as Kotlin default arguments, so only `mode` is required.

Generate bindings from the **debug** cdylib. `[profile.release]` sets
`strip = "symbols"`, and library-mode uniffi-bindgen reads exactly those
symbols — against a release build it silently produces nothing.

## Tests

`tests/host_roundtrip.rs` drives this surface as a foreign host would,
against the real committed 4.6 MB pack rather than a fixture. If a test
in that file ever needs `Pathfinder`, `CostConfig` or a port type, the
façade is leaking.
