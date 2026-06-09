# Synthetic UniFFI test clients

Dedicated clients that drive `place-core` through its **generated UniFFI
bindings** — not the Rust API — replaying the `golden.json` /
`golden_search.json` fixtures across the real FFI boundary. The point is to
prove the bindings (records, enums, the `PlaceEngine` object) lift/lower
correctly **before** wiring the core into the Flutter / Android / iOS apps.

The FFI surface is one object:

```
PlaceEngine.with_default_ruleset()            // embedded ruleset.v1.json
PlaceEngine.from_ruleset_json(json)           // runtime / bundle ruleset
engine.reverse_geocode(input) -> Description?
engine.forward_search(query, candidates) -> [Hit]
engine.ruleset_version() -> String
```

## Generating the bindings

```sh
cargo build --features uniffi
cargo run --features uniffi --bin uniffi-bindgen -- \
    generate --library target/debug/libplace_core.so \
    --language <python|kotlin|swift> --out-dir clients/<lang>
```

The committed `python/`, `kotlin/`, `swift/` binding files are a reviewable
snapshot of the surface; the runner below regenerates Python so it never drifts.

## Python — verified in CI / the dev container

The only client runnable here (Python 3 + ctypes; no extra toolchain):

```sh
./clients/run_python_client.sh
# OK — golden fixtures pass through the UniFFI Python binding
```

It builds the cdylib, regenerates the binding, drops `libplace_core.so` beside
it, and replays **every** golden case (23 reverse + 6 search) through the
binding.

## Kotlin / Swift — run where the toolchain exists

`GoldenClient.kt` and `place_core.swift` are dedicated clients for the Android
and iOS targets. They are **not** executed in the dev container (no `kotlinc` /
`swift` here); they assert a representative subset constructed via the generated
types. When the core lands in each app these become ordinary unit / instrumented
tests.

Kotlin (needs `kotlinc`, the JNA jar, and the cdylib on `java.library.path`):

```sh
cargo build --features uniffi
cargo run --features uniffi --bin uniffi-bindgen -- \
    generate --library target/debug/libplace_core.so --language kotlin --out-dir clients/kotlin
kotlinc clients/kotlin/uniffi/place_core/place_core.kt clients/kotlin/GoldenClient.kt \
    -cp jna.jar -include-runtime -d golden_client.jar
java -cp golden_client.jar:jna.jar -Djava.library.path=target/debug GoldenClientKt
```

Swift (needs the Swift toolchain): compile `clients/swift/place_core.swift`
against `place_coreFFI.modulemap` and link `libplace_core`, then call the same
`PlaceEngine` API.

## Why no Dart client here

Dart binds via `flutter_rust_bridge` (per the plan), not UniFFI, so its client
arrives with the Phase 0C Flutter wiring. The Python/Kotlin/Swift clients cover
the UniFFI surface that the Android + iOS targets use.
