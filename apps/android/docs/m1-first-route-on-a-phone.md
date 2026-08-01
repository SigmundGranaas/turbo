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

## No server required

The pack normally arrives from the tileserver at
`https://kart-api.sandring.no/v1/packs`. **That is not needed for this
test.** The APK carries one real region in its assets, so the whole
procedure runs with the radio off.

The bundled pack is `z12_2219_1001_2232_1014` — Sjunkhatten,
53 × 53 km, cut from the production artifacts with `tileserver
slice-pack`. Real terrain (552 DEM tiles) and a real trail graph
(4 293 nodes, 9 712 directed edges), not a fixture: the solver's cost
is a function of the graph it walks, so a toy graph would make the
phone look fast for a reason that would not generalise.

It is the region the artifacts cover, not the region you happen to be
in. That is fine — planning a route does not require standing in it,
and what is being measured is the engine.

It is **not installed automatically**: 55 MB of storage spent on one
region most users will never walk should be an explicit choice.

## The procedure

1. **Install** the `arm64-v8a` APK from the release (~69 MB, of which
   55 MB is the pack). It is signed with the committed sideload key, so
   it reinstalls over an earlier build without a signature conflict.

2. **Install the built-in region.** Settings → Routing engine →
   *Built-in region · Sjunkhatten · 53 × 53 km* → **Install**. That
   unpacks it into the same pack store a download would have written
   to, so everything downstream takes the identical path.

   Aeroplane mode is a good idea here: it proves the route is not
   quietly coming from the network.

3. **Force the phone.** Settings → Routing engine → **Phone**. This
   exists precisely because the default (Auto) is server-first: on a
   working connection the server answers every time and the device path
   is never reached, so a tester could route all day and measure
   nothing. Forcing it also **disables the fallback** — if the phone
   cannot answer, you see a failure instead of a server route quietly
   standing in for it.

4. **Plan a route** inside the region — anywhere around
   **15.03–16.26°E, 66.83–67.31°N**. Search for *Sjunkhatten* or pan
   there; the trail network is densest near its western edge, around
   15.04°E 67.07°N. The region's diagonal is ~75 km, so all three
   distance buckets are reachable.

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
| "No downloaded map covers this route" | the waypoints are outside the bundled region, or step 2 was skipped |

The first two should be impossible: `tools/verify-release-apk.py` runs
in the release workflow and fails the publish if the `.so` is absent,
the uniffi classes were stripped or renamed, or the bundled pack is
missing its manifest. If one appears anyway, the guard has a gap worth
fixing before anything else.

## When the tileserver comes back

Nothing here changes. The downloaded-pack path is untouched and still
the default route to a pack; the bundled region is an addition, and a
downloaded pack for another region sits beside it in the same store.
Remove the built-in one from the same settings row to get the 55 MB
back.

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
