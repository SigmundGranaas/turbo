# Serving and downloading routing packs

The UX plan's blocking dependency: `slice-pack` produces packs, nothing
distributes them. This is how.

**Short answer.** Serve them from the existing tileserver, through the
existing Cloudflare Worker and R2, as ordinary immutable GETs — no new
infrastructure. Download them as one more *lane* in the offline
downloader the app already has. The work is in three places: a
deterministic name for a pack, a build trigger on the origin, and one
change to the Worker that packs would otherwise break.

## Where: nothing new

The pieces already exist and fit.

- **Origin:** the tileserver (`kart-api.sandring.no`), which already
  mmaps the 14 GB national artifacts off a hostPath PV and already has
  the slicing code compiled in.
- **Edge:** `infra/edge/tiles-worker` — a Worker with R2 as a
  pull-through cache, keyed by `DATA_VERSION`, with the design rule that
  *every R2 object is byte-for-byte regenerable from origin* and R2 is
  never the system of record.
- **Client:** `WgpuOfflineTileManager`, which is already a
  list-of-URLs-into-a-store engine with progress, pause/resume, network
  gating, and a foreground service.

The R2 tier matters more for packs than for tiles: a pack is megabytes
where a tile is kilobytes, the origin is a single k3s node, and
Cloudflare charges no egress from R2. Packs are the traffic that would
hurt most served directly and benefit most from the cache that is
already there.

## The naming problem, and why it decides the design

The Worker's invariant — regenerable from origin, keyed by path — means
**a pack's URL must be deterministic and finite.** A free-form
`?bbox=14.9013,67.0217,...` fails both: two users framing the same valley
produce two keys, cache hit rate collapses, and R2 fills with near-
duplicates.

So the bbox is **quantised**: snap the request outward to a fixed grid
and name the pack by its grid extent.

```
/v1/packs/{data_version}/z12/{x0}_{y0}_{x1}_{y1}/norway.dem
                                                 /norway.mask
                                                 /norway.graph
                                                 /norway.graph_geom
                                                 /pack.toml
```

`z12/x/y` is the slippy-tile grid the app and the tile pipeline already
think in. At Norwegian latitudes a z12 cell is **3.8 km at 67°N, 4.9 km
at 60°N** — fine enough that snapping wastes little, coarse enough that a
typical region is a handful of cells. A 13 × 13 km viewport snaps to
4 × 4 cells ≈ 15 × 15 km.

The client computes the key itself: pure arithmetic on the viewport, no
round trip to ask what a region is called.

### Why one pack per region, not one pack per tile

Tempting to make a pack *be* a z12 tile — perfect cache behaviour,
trivially finite key space. It breaks on the engine: `RouteEngine::open`
binds one pack directory and `PackStore.covering` requires a single pack
to contain every waypoint. With 3.8 km packs, most real routes straddle
two and get "no downloaded map covers this route" while sitting in the
middle of a downloaded area.

Multi-pack routing is the better long-term answer and the §9 manifest
design anticipated it (`[[source]]` entries). It is engine work, not
distribution work, and it should not gate shipping. **Region-sized packs
on a quantised grid** gets the caching benefit without it.

The cost is honest and worth stating: overlapping regions duplicate bytes
in R2. For a pull-through cache with lifecycle expiry that is acceptable
— it is warmth, not truth.

## Building: on the origin, on demand, once

**Trigger:** the first `GET .../pack.toml` for a key builds the pack.

The manifest is tiny, so a slow response there is a "preparing your
area…" moment rather than a stalled multi-megabyte download. By the time
the client asks for `norway.dem`, the bytes are on disk and stream at
line rate.

**If the build runs long, return `202` + `Retry-After`** and let the
client poll the same URL. This is the standard shape, needs no second
endpoint, and only engages for large regions.

**Concurrency:** one build per key (a keyed lock — a second request for
the same key waits rather than duplicating the work), and a global
permit cap, the same pattern `route_plan.rs` already uses with
`acquire_routing_permit`. Plus a hard area cap, because a public
endpoint will be asked for all of Norway eventually.

### What a build costs

Measured here, against a 34.8 MB regional source:

| region | slice time | pack |
|---|---:|---:|
| 12.9 × 13.2 km | 2.4 s | 2.6 MB |
| 21 × 28 km | 4.2 s | 10.7 MB |

**These numbers do not transfer, and that is the main open risk.** The
DEM slice is a tile-directory filter and a byte copy, so it scales with
*output*. The graph slice reads every node and edge and rebuilds the CSR,
so it scales with *input* — and the input here was 9,642 edges against a
national graph that is orders of magnitude larger. Pack build time on
production artifacts could be seconds or minutes, and the answer changes
the design: seconds means build-on-demand as described; minutes means
pre-building popular regions is mandatory rather than an optimisation.

**Measure this first.** It is one command against the staged artifacts on
the node, and it is the cheapest way to de-risk the whole plan.

If it turns out slow, the graph slice can be made output-scaled with a
spatial index over nodes — but do not do that work speculatively.

### Storage

Built packs cache on the origin's disk under `{data_version}/{key}/`,
with an LRU sweep.

**The PV has no room.** `tileserver-artifacts` is 20 Gi with ~14 Gi of
artifacts — about 6 Gi of headroom, which a few dozen packs would eat.
Packs want their own PVC, sized for the cache rather than the source
data, and separate so a full pack cache can never wedge the artifacts the
router itself mmaps.

**`DATA_VERSION` couples correctly already.** A data rebuild bumps it,
which orphans both tile and pack keys — old packs lifecycle-expire and
new requests rebuild from the new artifacts. Packs stay consistent with
the basemap for free.

## Serving: individual files, not an archive

A pack is 4–5 files. Ship them as 4–5 GETs, not one tar or zip.

- **It reuses the client's existing machinery unchanged.** The offline
  downloader is already "enumerate URLs, fetch each into a store, stream
  progress, skip what is already on disk on resume". A pack is a short
  URL list. An archive needs a new unpacking step, a new progress model,
  and its own resume story.
- **Each file is independently cacheable.** R2 pull-through works per
  object, exactly as for tiles.
- **Compression buys nothing.** DEM tiles are already zstd internally;
  gzipping them again costs CPU on both ends for a few percent.

Atomicity comes from the filesystem, not the transport: download into
`packs/{key}.partial/` and rename on completion. `PackStore` only counts
a directory as a pack if it has a `pack.toml`, so a half-finished
download is invisible to routing even if the rename never happens.

## The Worker change packs would otherwise break

`worker.js` does `await upstream.arrayBuffer()` before writing through to
R2 — **the whole response buffered in memory.** Cloudflare Workers cap at
128 MB. A 35 MB pack DEM survives alone and does not survive concurrency;
it is a latent OOM that tiles never trip because tiles are kilobytes.

Two options, in order of preference:

1. **Stream it.** `body.tee()` gives two readable streams — one to the
   client, one to `TILES.put()`. R2 accepts a stream, memory stays flat,
   and it improves the tile path too by removing a needless buffer.
2. **Cap it.** Above some size, proxy through without caching. One line,
   correct, and costs the CDN benefit on exactly the objects that need it
   most.

Do (1). Do not add pack paths to `CACHEABLE` before doing it.

The allowlist gains one pattern:

```js
/^\/v1\/packs\/[\w.-]+\/z\d+\/\d+_\d+_\d+_\d+\/[\w.]+$/,
```

Note it matches the *whole* pack path including `data_version`, which is
already in the R2 key — harmless duplication, and it keeps the URL
self-describing for anyone debugging with curl.

## Downloading: one more lane

Client-side this is small, because `WgpuOfflineTileManager` already does
the hard parts. A pack becomes a lane whose "tiles" are five files and
whose store is a directory instead of a `TileStore`.

What rides for free: progress, pause/resume, the network policy that
pauses on metered connections, the foreground service and its
notification, region persistence across relaunch, and delete.

**One rule must change.** Resume today is "skip what is on disk", which
is right for a 20 KB tile that is either there or not, and **wrong for a
35 MB DEM** that may be half-written. Pack files need a size or ETag
check, not an existence check — otherwise a download interrupted mid-DEM
resumes into a truncated file, and the failure surfaces later as a pack
that opens and routes wrongly. Range requests then make resume genuinely
incremental rather than restart-the-file.

**Progress needs the sizes up front**, which is what makes `pack.toml`
being fetched first do double duty: it is the build trigger *and* it
carries the file list and byte counts the progress bar needs. It should
gain a `[[file]] name = "…" bytes = …` section for that. (It should
probably also carry a checksum per file, which is the honest way to
detect the truncation case above.)

**Failure granularity matters more than for tiles.** A missing tile is
legitimate — `FetchOutcome.Absent` exists because an ocean DEM tile is
genuinely absent. A missing pack file is never legitimate. Pack fetches
must treat 404 as an error, not as an empty tile, or a broken pack will
be marked complete.

## Sequencing

1. **Measure the slice on production artifacts.** One command on the
   node. Everything below assumes seconds; if it is minutes, insert a
   pre-build step and revisit.
2. **`pack.toml` gains the file list, sizes, and checksums.** Small
   change to `slice_pack::write_manifest` and the `PackManifest` struct,
   and it unblocks client progress.
3. **Origin endpoint:** quantise, keyed build lock, disk cache, 202 for
   long builds. Its own PVC.
4. **Worker:** `tee()` streaming write-through, then the `CACHEABLE`
   pattern. In that order.
5. **Client lane:** pack URLs, per-file size/checksum verification,
   partial-dir rename, 404-is-an-error.
6. **Wire it into `DownloadSpec`/`OfflineEstimate`** so the download
   dialog's size includes the pack — the first user-visible step, and the
   one the UX plan starts from.

## Alternatives considered

**Pre-build the whole country.** Norway at z12 is ~25,000 cells; as
region packs it is not enumerable at all. As per-tile packs it is
buildable (~6 GB) but reintroduces the straddling problem and needs
multi-pack routing first. Revisit if on-demand build proves too slow —
and then only for popular regions, not the whole country.

**Serve packs straight from origin, no Worker.** Simplest possible, and
it works. It also puts every megabyte on a single node behind a home-lab
ingress, which is the one traffic shape this system's edge tier exists to
avoid.

**A separate pack service.** More moving parts, another deploy, another
PVC, and it would need its own copy of the artifacts — the tileserver
already has them mmapped and the slicing code linked in.

**Build packs in CI and ship them as release assets.** Deterministic and
free to serve, but it decouples pack data from the deployed artifacts,
which is exactly the drift `DATA_VERSION` exists to prevent.
