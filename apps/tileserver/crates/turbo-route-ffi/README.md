# turbo-route-ffi

The on-device routing engine, as Kotlin and Swift see it.

```kotlin
val engine = RouteEngine("/data/data/.../packs/sjunkhatten")
if (!engine.hasCoverage(here)) { promptDownload(); return }

val route = engine.plan(listOf(here, there), RouteOptions(
    mode = TravelMode.FOOT,
    preset = "balanced",
    forceOffTrail = false,
    maxSpanKm = 100.0,
))
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

```sh
tileserver slice-pack \
  --src ~/.data/artifacts --dst ./packs/sjunkhatten \
  --corpus tools/sjunkhatten-ci-corpus.toml --halo-m 1000
```

A pack is a directory of `norway.{dem,mask,graph,graph_geom}` — the same
formats the server reads, so there is no separate pack reader. The
Sjunkhatten CI pack is **4.6 MB** for 14 × 15 km.

Only the DEM is required. Without a graph, routes are cross-country;
without a mask, water and glaciers are not refused. A missing DEM is
fatal, because routing without terrain would return a straight line —
which the codebase elsewhere calls "semantically a lie".

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

## Two things a host must handle

**`maxSpanKm` is load-bearing, not advisory.** The engine's internal
guard (`max_off_trail_km`) bounds only the cross-country mesh; a request
that can reach the trail network is bounded by nothing. Writing this
crate's tests found the consequence: a route from Oslo to a Sjunkhatten
pack — 850 km, one endpoint outside coverage entirely — **solved, in 83
seconds**. On a server that is a slow request. On a phone it is an ANR
and a flat battery. The default (100 km) is generous for a day's walk and
cheap to raise deliberately.

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

**Still open:** E1 settled glibc-vs-glibc under QEMU. Bionic on real
silicon is unverified — run `tools/experiments/e1_crossisa` on a device
before relying on server/device agreement.

## Tests

`tests/host_roundtrip.rs` drives this surface as a foreign host would,
against the real committed 4.6 MB pack rather than a fixture. If a test
in that file ever needs `Pathfinder`, `CostConfig` or a port type, the
façade is leaking.
