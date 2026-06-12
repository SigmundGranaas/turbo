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

## One command: `ci.sh`

Runs every verifiable layer — pure Rust golden tests, the Python client, and
(when a Kotlin toolchain + JNA are present) the Kotlin client:

```sh
./clients/ci.sh                              # Rust + Python (Kotlin auto-skipped)
KOTLINC=/path/to/kotlinc JNA_JAR=/path/to/jna-5.x.jar ./clients/ci.sh   # + Kotlin
```

Each client replays the golden cases through the binding, then exercises the
`from_ruleset_json` **round-trip** (an engine built from the ruleset JSON must
match the embedded one) and the **invalid-ruleset error path** (typed
`EngineError` / `EngineException`, no crash).

## Python — verified

Standalone runner (Python 3 + ctypes, no extra toolchain):

```sh
./clients/run_python_client.sh
# OK — golden fixtures + round-trip + error path pass through the UniFFI Python binding
```

Replays **every** golden case (23 reverse + 6 search) through the binding.

## Kotlin — verified

`GoldenClient.kt` is the Android target's client. It asserts a representative
subset (full fixture parity is the Python client's job) plus the round-trip and
error paths. Verified on JDK 21 + Kotlin 2.0 + JNA 5.14. Note JNA resolves the
cdylib via **`jna.library.path`** (not `java.library.path`):

```sh
cargo build --features uniffi
cargo run --features uniffi --bin uniffi-bindgen -- \
    generate --library target/debug/libplace_core.so --language kotlin --out-dir clients/kotlin
kotlinc clients/kotlin/uniffi/place_core/place_core.kt clients/kotlin/GoldenClient.kt \
    -cp jna.jar -include-runtime -d gc.jar
java -cp gc.jar:jna.jar -Djna.library.path="$PWD/target/debug" GoldenClientKt
```

When the core lands in the Android module this becomes an ordinary unit test.

> The Kotlin compile caught a real binding bug Python couldn't: an `EngineError`
> variant field named `message` collides with `Throwable.message` in the
> generated error class — hence the field is `reason`. Verifying each binding
> against its real toolchain matters.

## Swift — generated, not yet executed

`place_core.swift` is generated for the iOS target but isn't compiled here (no
Swift toolchain in the container). Compile it against `place_coreFFI.modulemap`,
link `libplace_core`, and call the same `PlaceEngine` API; it becomes an XCTest
when the core lands in the iOS app.

## Why no Dart client here

Dart binds via `flutter_rust_bridge` (per the plan), not UniFFI, so its client
arrives with the Phase 0C Flutter wiring. The Python/Kotlin/Swift clients cover
the UniFFI surface that the Android + iOS targets use.
