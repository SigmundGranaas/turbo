# M2 — how long does cutting a pack take at national scale?

`GET /v1/packs/:key/:file` cuts a region out of the national artifacts
on demand and answers `202 Retry-After` if the cut outlives
`BUILD_WAIT`. That design is only sound if a cut is **seconds**. If it
is minutes, build-on-demand is not a cache miss, it is an outage, and
popular regions have to be pre-built — a different endpoint, a
different deployment, a different plan.

**Answer: seconds. The design holds.** An ordinary app-sized region is
1–3 s against national artifacts, well inside the 20 s window. But the
measurement turned up two defects that were invisible without it, and
both are fixed here.

## How it was measured

The production artifacts live on the k3s node and are not reachable
from this container. That is survivable, because this is a *scaling*
question and the phases scale on separate, independently controllable
axes:

| phase | reads | scales with |
|---|---|---|
| dem | the tile directory, then copies the kept tiles | source **tile count**; output bytes |
| mask | the whole raster payload, crops it | source **cell count** |
| graph | every node and every edge, rebuilds the CSR | source **edge count** |
| verify | re-opens the source DEM (r-tree over every tile) | source **tile count** |

Two of the four need no extrapolation at all:

- **The mask in `.data/artifacts` is already national.** 13411 × 20707
  cells at 100 m is 1341 × 2071 km — the whole country. Its number is a
  measurement.
- **The output terms** (tile copy, digest) depend on the region asked
  for, not on how big the source is, and the region asked for is the
  same either way.

Only the DEM tile count and the graph edge count needed growing.
`cargo run --release -p turbo-geodata-pack --example scaling` does that:
it synthesises sources at a parameterised scale, hard-links the real
national mask, and sweeps each axis separately.

Norway's mainland is ~324 000 km²; a 256-cell tile at 10 m covers 6.55
km², so a complete DTM10 pyramid is **~49 400 tiles**. Scaling the
Sjunkhatten graph (157 074 directed edges over 8 666 km²) by area gives
**~5.9 M edges**.

## Results

Per-phase timing was added to `Report` for this and kept, because the
sizes alone cannot say which phase to fix.

**Baseline, the real 209 MB regional source, region 14.2 × 13.2 km:**

```
  dem      118.3 MB ->   3.7 MB   0.01 s   36 / 1360 tiles
  mask      69.4 MB ->   0.1 MB   0.05 s   568x528 of 13411x20707 cells
  graph     30.8 MB ->   0.2 MB   0.03 s   490 / 70322 nodes, 1044 / 157074 edges
  verify                          0.04 s   4096 points
  manifest                        0.02 s
  TOTAL    218.6 MB ->   3.9 MB   0.16 s
```

**Marginal costs, from the sweeps (warm; linear in both axes across 5
and 4 points, over two independent runs):**

- per source DEM tile (dem + verify): **2.7–4.2 µs**
- per source graph edge: **0.19–0.22 µs**
- source *bytes* are free: 10 000 tiles at 1 MB total and at 767 MB
  total cut in 0.02 s and 0.03 s. The copy loop touches only the kept
  tiles; the rest of the file is never read.

The ranges are run-to-run noise on a shared box, which is why the
national figure below is measured directly rather than extrapolated
from these. Both runs' extrapolations (1.3 s and 1.6 s) bracket the
measured 1.73 s, which is the check that the sweeps model the right
thing.

**National synthetic source (49 438 tiles, 2.5 M nodes, 5.87 M edges, 1.8 GB):**

```
  NATIONAL warm   dem 0.11  mask 0.16  graph 1.39  verify 0.05  TOTAL 1.73 s
  NATIONAL cold   dem 0.13  mask 0.12  graph 2.95  verify 0.06  TOTAL 3.26 s
  NATIONAL cold   dem 0.12  mask 0.18  graph 2.17  verify 0.06  TOTAL 2.52 s
```

**The portable number: 321 MB read from disk per build** (`/proc/self/io`
`read_bytes`, deterministic across runs; 110 MB against the regional
source). Wall time here is a property of one container on one VM whose
*host* cache the guest cannot drop. Bytes-read is a property of the
algorithm — divide by the k3s node's read throughput for its number.

The graph phase dominates, and it is the one phase that reads its whole
input: every node (20 MB) and every edge (188 MB) is walked regardless
of how few are kept.

### Caveats

- Warm/cold matters enormously: the real source cut in 4.51 s on a
  genuinely cold cache and 0.16 s warm, a 28× gap. A server that has
  just started pays the cold price once per artifact and then does not.
- The synthetic DEM uses 29 KB tile payloads rather than the real 87 KB
  (disk headroom). That shortens the seeks between the tiles the slice
  copies, biasing the cold DEM number optimistic. It is the one term
  this container cannot measure faithfully.

## What the measurement found

### 1. The size cap was set in the wrong units

`MAX_CELLS = 400`, documented as "roughly 76 × 76 km at 67°N". The
description was accurate and the cap was still wrong. A z12 cell is
square *on the ground* at every latitude — the cos(lat) that shrinks a
degree of longitude is the same one that stretches the Mercator y scale
— but its size still falls with latitude: 5.2 km a side at 58°N, 3.8 km
at 67°N. The same 400 cells are **5 842 km² at 67°N and 11 017 km² at
58°N**, and Norway runs 58 to 71. The cap was nearly twice as loose
where most of the country is.

Measured directly: cutting 84.9 × 99.0 km (8 405 km²) out of the real
source took **12.0 s** and produced **122.4 MB** — 14.6 KB/km². Scaled
to 11 017 km² and with the graph phase against national artifacts, a
southern request at the old cap was a **~161 MB pack and ~18 s of
build**: inside `BUILD_WAIT` by 10%, and a download no phone on a
mountain connection should be handed as one indivisible unit.

**Fixed:** the cap is now `MAX_AREA_SQ_KM = 5_500.0`, computed from the
key's own extent, with the cell count demoted to a cheap pre-filter
against malformed keys. 5 500 km² keeps the build near 11 s and the
pack near 80 MB at *every* Norwegian latitude. It is deliberately close
to what 400 cells meant at 67°N — the intent behind the old constant
was right; only its units were not.

`PackKey::area_sq_km` carries the doc; `packs.rs` has three tests, one
of which (`the_cap_admits_the_same_work_at_every_norwegian_latitude`)
was mutation-verified to fail against the old cell cap with a 2.6×
spread.

### 2. An oversized region failed the whole map download

The app's own guard is `MAX_SPAN_DEGREES = 6.0` — far looser than the
server's cap. A region between the two is legitimate: it downloads as a
map and cannot be one routing pack. What happened instead: the pack
endpoint answered `400`, `okHttpPackFetcher` mapped any non-404 failure
to `FetchResult.Failed`, `PackDownloader` returned `Outcome.Failed`, and
`WgpuOfflineTileManager` called `markFailed` — **taking the map tiles
down with it**.

That is the same class of bug `Outcome.Unsupported` was created for
(the missing-endpoint case), and it was live.

**Fixed, on both sides of the wire:**

- `RoutingPack.MAX_AREA_SQ_KM` / `areaSqKm` / `fitsOnePack` mirror the
  server's rule, so the client never asks for what would be refused.
  The two formulas are pinned to shared reference values —
  `area_reference_values_for_the_android_side` in Rust,
  `RoutingPackAreaTest` in Kotlin — so drifting one without the other
  fails a test.
- `PackDownloader.Outcome.TooLarge`, treated like `Unsupported`: the
  region completes with tiles and no pack. `FetchResult.TooLarge` maps
  a `400` the same way, for the app/server pair that is not matched.
- `OfflineEstimate.routingOmittedForSize` — explicit rather than
  inferred from `packBytes == 0`, because that is also what "routing
  not requested" looks like. The download dialog now says *"Too large
  for offline routing — the map still downloads"* instead of silently
  dropping the "includes routing" line.

## What this means for the server's constants

- **`BUILD_WAIT = 20 s` holds.** An ordinary region is 1–3 s; the
  largest region now permitted is ~11 s. Unchanged.
- **`TILESERVER_PACK_BUILD_CONCURRENCY` defaults to 2, and should stay
  low.** Each build reads ~320 MB and holds the whole 69 MB mask payload
  in memory (`read_to_end`) while cropping it. On a 1–4 GB box, a high
  concurrency is an OOM, not a throughput win.
- **Pre-building popular regions is not required.** That was the fork
  this task existed to resolve, and the answer is that on-demand works
  as designed.

## Cheap wins not taken

Left deliberately, since the answer is "fast enough" and each is a
behaviour risk out of proportion to its gain:

- `slice_dem` reads the tile directory through an **unbuffered**
  `File`, and `read_tile_entry` does five reads per entry — 247 000
  syscalls at national scale. Wrapping it in a `BufReader` is a
  one-line change worth roughly 0.1 s.
- `slice_graph` walks all 5.9 M edges to keep ~1 000. A coarse spatial
  index over nodes would make it output-proportional and remove ~1.4 s,
  which is most of the national cut. Worth doing if the graph grows
  past Norway.
