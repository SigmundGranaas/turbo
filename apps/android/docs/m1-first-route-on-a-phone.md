# M1 — running the first real route, from the published APK

Everything about on-device routing is verified except the part that
matters: **it has never run on a phone.** Route hashes match across
x86_64/glibc, aarch64/glibc and aarch64/bionic (that last under
qemu-user, so determinism is settled), but every *latency* figure quoted
for the device path is extrapolated from a desktop, and no APK built by
CI has ever had its native library loaded by ART.

This is the procedure for closing that, using the release APK from a
GitHub Release rather than a debug build. The distinction is the whole
point — see "Why not a debug build" below.

## Prerequisite: the tileserver must serve packs

The phone downloads its routing pack from
`https://kart-api.sandring.no/v1/packs`. That endpoint ships in this
same branch (P2/P3), so **the tileserver has to be deployed before the
APK is useful.** If it is not, the symptom is quiet and easy to
misread: the pack request 404s, `PackDownloader` returns
`Outcome.Unsupported`, the region downloads as a map with no pack, and
forcing the phone then reports *"No downloaded map covers this route."*
That is a deployment state, not a routing defect.

Check it before installing anything:

```
curl -sI https://kart-api.sandring.no/v1/packs/z12_2218_1006_2221_1009/pack.toml
```

`200` (or `202`, meaning "building, come back") is ready. `404` means
the deployed tileserver predates the pack endpoint.

## The procedure

1. **Install** the `arm64-v8a` APK from the release. It is signed with
   the committed sideload key, so it reinstalls over an earlier build
   without a signature conflict.

2. **Download a region containing trails.** Map → layers sheet →
   "Download this area". The dialog says either *"Includes trail
   routing"* or *"Too large for offline routing"* — you need the first,
   so zoom in until you see it. Regions over 5 500 km² get a map and no
   pack, by design.

3. **Force the phone.** Settings → Routing engine → **Phone**. This
   exists precisely because the default (Auto) is server-first: on a
   working connection the server answers every time and the device path
   is never reached, so a tester could route all day and measure
   nothing. Forcing it also **disables the fallback** — if the phone
   cannot answer, you see a failure instead of a server route quietly
   standing in for it.

4. **Plan a route** inside the downloaded region.

5. **Read the numbers.** Settings → Routing engine, below the picker:
   one row per solve, newest first —

   ```
   device   1,840 ms   4.2 km · 2      ok
   ```

   engine · wall time · straight-line span and waypoint count · outcome.
   Span is what latency has to be reported against; a 2 km solve and a
   40 km one are different questions.

6. **Get a control.** Switch to **Server**, plan the same route, and
   compare. That is the number the device time has to beat, or at least
   not embarrass itself against.

7. **Set it back to Auto** when you are done.

### What to write down

For each distance bucket you can reach — roughly 1–5 km, 5–15 km,
15 km+ — a handful of device solves and a matching server solve.
Device p95 by bucket is the figure release 2 rests on, and the point at
which the device path becomes the default rather than the fallback.

### If a solve says `failed`

The row carries the message underneath it, and on a release APK the
likely causes are specific:

| message mentions | means |
|---|---|
| `libturbo_route_ffi.so` / `UnsatisfiedLinkError` | the native library is missing for this ABI, or R8 stripped the bindings |
| `capacity` / `FieldOrder` / a JNA reflection error | R8 renamed the JNA struct fields — the `-keep` rules regressed |
| "No downloaded map covers this route" | no pack for these points; step 2 did not produce one (see the prerequisite) |

The first two should be impossible: `tools/verify-release-apk.py` runs
in the release workflow and fails the publish if the `.so` is absent or
the uniffi classes were stripped or renamed. If one appears anyway, the
guard has a gap worth fixing before anything else.

## Why not a debug build

Three release-only differences, each of which can break routing on its
own and none of which a debug build exercises:

- **R8 runs.** `isMinifyEnabled = true`. JNA resolves struct fields by
  *name*, from strings in `@Structure.FieldOrder`; renaming the fields
  leaves the strings intact and breaks the binding at the first Rust
  call. Debug does not minify, so every on-device route run before this
  was against un-minified bindings.
- **The APK is split by ABI.** Release produces one APK per ABI; debug
  produces a single all-ABI APK. A mismatch between the ABI list in
  `:core:routing-android` and the `splits` block ships an APK with no
  routing library — which installs and launches perfectly.
- **It is the artifact users get.** Same signing key, same shrinking,
  same packaging.

## What this is not

The readout is a diagnostic, not telemetry. It lives in memory, holds
the last 20 solves, uploads nothing, and dies with the process. U3
replaces it with something aggregated and durable; until then it is
what stops release 2 resting on a hunch.
