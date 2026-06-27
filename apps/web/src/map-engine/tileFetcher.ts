import type { TurboMap } from 'turbomap-web';
import { type TileKind, type Templates, tileUrl } from './templates';

interface PendingTile {
  kind: TileKind;
  layer: string;
  z: number;
  x: number;
  y: number;
}

/** Per-tile-kind concurrency lanes. The DEM ("terrain") server is much slower
 *  than the imagery CDN (~1.5 s vs ~0.2 s/tile) and 3D needs the DEM for the
 *  relief, so it gets its own lanes and isn't starved when the fast imagery
 *  floods the queue. Different hosts → independent connection pools. */
const LANES: Record<TileKind, number> = { terrain: 12, raster: 16, vector: 8, hillshade: 4 };

/** Static tiles worth a read-through cache (immutable: elevation + topo). */
const CACHEABLE = new Set<TileKind>(['terrain', 'raster']);
const TILE_CACHE = 'turbo-tiles-v1';

/** Drives the host side of the engine's host-driven tile IO: read
 *  `pending_tiles()` (already globally ordered near/in-front first by the
 *  engine), fetch each missing tile, and push the bytes back through the
 *  matching `ingest_*`.
 *
 *  Three behaviours make 3D loading feel responsive:
 *  - **Preemption**: each pump, in-flight tiles the engine no longer wants
 *    (e.g. after a zoom/pan) are ABORTED, freeing lanes for the new near tiles
 *    instead of waiting behind stale far work.
 *  - **Per-kind lanes**: the slow DEM gets a reserved budget so the fast
 *    imagery can't starve it.
 *  - **Cache read-through** (Cache API): immutable tiles (DEM/topo) are served
 *    from cache on revisit — instant, and no repeat hits on the rate-limited
 *    DEM server. */
export class TileLoader {
  private inflight = new Map<string, { ctrl: AbortController; kind: TileKind }>();
  private cache: Promise<Cache | null>;
  stored = 0;
  failed = 0;

  constructor(
    private readonly map: TurboMap,
    private templates: Templates,
  ) {
    this.cache =
      typeof caches !== 'undefined'
        ? caches.open(TILE_CACHE).catch(() => null)
        : Promise.resolve(null);
  }

  setTemplates(t: Templates): void {
    this.templates = t;
  }

  /** Kick off fetches for tiles the engine wants and isn't already loading,
   *  and cancel in-flight tiles it no longer wants. Cheap to call every frame. */
  pump(): void {
    let pending: PendingTile[];
    try {
      pending = JSON.parse(this.map.pending_tiles()) as PendingTile[];
    } catch {
      return;
    }
    const key = (t: PendingTile) => `${t.kind}/${t.layer}/${t.z}/${t.x}/${t.y}`;

    // Preempt: abort in-flight tiles the engine no longer wants. After a zoom
    // or pan the desired set changes; without this the lanes stay full of stale
    // far tiles and the new near ones wait behind them.
    const wanted = new Set(pending.map(key));
    for (const [k, v] of this.inflight) {
      if (!wanted.has(k)) {
        v.ctrl.abort();
        this.inflight.delete(k);
      }
    }

    // Fill lanes per kind, in the engine's near/in-front-first order.
    const used: Record<string, number> = {};
    for (const [, v] of this.inflight) used[v.kind] = (used[v.kind] || 0) + 1;
    for (const t of pending) {
      const cap = LANES[t.kind] ?? 8;
      if ((used[t.kind] || 0) >= cap) continue;
      const k = key(t);
      if (this.inflight.has(k)) continue;
      const template = this.templates[t.kind]?.[t.layer];
      if (!template) continue;
      const ctrl = new AbortController();
      this.inflight.set(k, { ctrl, kind: t.kind });
      used[t.kind] = (used[t.kind] || 0) + 1;
      void this.fetchOne(t, tileUrl(template, t.z, t.x, t.y), k, ctrl.signal);
    }
  }

  private ingest(t: PendingTile, bytes: Uint8Array): void {
    switch (t.kind) {
      case 'raster':
      case 'hillshade':
        this.map.ingest_raster_tile(t.layer, t.z, t.x, t.y, bytes);
        break;
      case 'terrain':
        this.map.ingest_terrain_tile(t.z, t.x, t.y, bytes);
        break;
      case 'vector':
        this.map.ingest_vector_tile(t.layer, t.z, t.x, t.y, bytes);
        break;
    }
    this.stored++;
  }

  private async fetchOne(t: PendingTile, url: string, key: string, signal: AbortSignal): Promise<void> {
    const cacheable = CACHEABLE.has(t.kind);
    try {
      const cache = cacheable ? await this.cache : null;
      // Cache-first for immutable tiles → instant on revisit, no DEM 429s.
      if (cache) {
        const hit = await cache.match(url);
        if (hit) {
          this.ingest(t, new Uint8Array(await hit.arrayBuffer()));
          return;
        }
      }
      const res = await fetch(url, { signal });
      // 204/404 = no tile here (ocean, out of coverage). Absent, not a failure.
      if (!res.ok) {
        this.failed++;
        return;
      }
      if (cache) void cache.put(url, res.clone()).catch(() => {});
      this.ingest(t, new Uint8Array(await res.arrayBuffer()));
    } catch (e) {
      // Aborted (preempted) fetches are expected, not failures.
      if (!(e instanceof DOMException && e.name === 'AbortError')) this.failed++;
    } finally {
      this.inflight.delete(key);
    }
  }
}
