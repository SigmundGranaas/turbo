import type { TurboMap } from 'turbomap-web';
import { type TileKind, type Templates, tileUrl } from './templates';

interface PlanFetch {
  id: number;
  kind: TileKind;
  layer: string;
  z: number;
  x: number;
  y: number;
}

interface StreamingPlan {
  start: PlanFetch[];
  cancel: number[];
}

/** Per-tile-kind concurrency lanes. The DEM ("terrain") server is much slower
 *  than the imagery CDN (~1.5 s vs ~0.2 s/tile) and 3D needs the DEM for the
 *  relief, so it gets its own lanes and isn't starved when the fast imagery
 *  floods the queue. Different hosts → independent connection pools.
 *  (These migrate into the engine's streaming budgets in plan slice B4.) */
const LANES: Record<TileKind, number> = { terrain: 12, raster: 16, vector: 8, hillshade: 4 };

/** Static tiles worth a read-through cache (immutable: elevation + topo). */
const CACHEABLE = new Set<TileKind>(['terrain', 'raster']);
const TILE_CACHE = 'turbo-tiles-v1';

/** The first full plan-driven host (slice B3.3). Each pump consumes ONE
 *  engine `streaming_plan`:
 *  - `start` — priority-ordered fetches, each with a RequestId; the engine
 *    never hands the same attempt out twice.
 *  - `cancel` — in-flight attempts the camera moved away from: their
 *    transports are ABORTED and reported back, freeing lanes for near work.
 *    (This decision used to be re-derived here by diffing pending lists;
 *    the engine's lifecycle table now states it.)
 *  Deliveries complete through the ordinary `ingest_*`; failures and
 *  declined/aborted starts are reported so the engine can re-issue them.
 *  Cache read-through (Cache API) is kept for immutable tiles — instant
 *  revisits, no repeat hits on the rate-limited DEM server. */
export class TileLoader {
  private inflight = new Map<number, { ctrl: AbortController; kind: TileKind }>();
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

  /** Take one plan sized to the free lane capacity, honour its cancels, and
   *  start its fetches. Cheap to call every frame. */
  pump(): void {
    const used: Record<string, number> = {};
    for (const [, v] of this.inflight) used[v.kind] = (used[v.kind] || 0) + 1;
    const freeLanes = Object.entries(LANES).reduce(
      (sum, [kind, cap]) => sum + Math.max(0, cap - (used[kind] || 0)),
      0,
    );

    let plan: StreamingPlan;
    try {
      plan = JSON.parse(this.map.streaming_plan(freeLanes)) as StreamingPlan;
    } catch {
      return;
    }

    for (const id of plan.cancel) {
      const v = this.inflight.get(id);
      if (v) {
        v.ctrl.abort();
        this.inflight.delete(id);
      }
      // Report regardless: an unknown id (e.g. after a reload race) must
      // still be acknowledged or the engine will keep listing it.
      this.map.report_fetch_cancelled(id);
    }

    for (const t of plan.start) {
      // Lanes are per-kind but the plan budget is global, so a start can
      // land on a full lane; decline it so the engine re-issues it next
      // plan instead of it being stuck as a phantom in-flight attempt.
      if ((used[t.kind] || 0) >= (LANES[t.kind] ?? 8)) {
        this.map.report_fetch_cancelled(t.id);
        continue;
      }
      const template = this.templates[t.kind]?.[t.layer];
      if (!template) {
        this.map.report_fetch_cancelled(t.id);
        continue;
      }
      const ctrl = new AbortController();
      this.inflight.set(t.id, { ctrl, kind: t.kind });
      used[t.kind] = (used[t.kind] || 0) + 1;
      void this.fetchOne(t, tileUrl(template, t.z, t.x, t.y), ctrl.signal);
    }
  }

  private ingest(t: PlanFetch, bytes: Uint8Array): void {
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

  private async fetchOne(t: PlanFetch, url: string, signal: AbortSignal): Promise<void> {
    const cacheable = CACHEABLE.has(t.kind);
    let outcome: 'delivered' | 'failed' | 'aborted' = 'failed';
    try {
      const cache = cacheable ? await this.cache : null;
      // Cache-first for immutable tiles → instant on revisit, no DEM 429s.
      if (cache) {
        const hit = await cache.match(url);
        if (hit) {
          this.ingest(t, new Uint8Array(await hit.arrayBuffer()));
          outcome = 'delivered';
          return;
        }
      }
      const res = await fetch(url, { signal });
      // 204/404 = no tile here (ocean, out of coverage). Absent, not a
      // failure — but the attempt still ends, so it reports as failed and
      // the engine's want-set decides whether to retry.
      if (!res.ok) {
        this.failed++;
        return;
      }
      if (cache) void cache.put(url, res.clone()).catch(() => {});
      this.ingest(t, new Uint8Array(await res.arrayBuffer()));
      outcome = 'delivered';
    } catch (e) {
      if (e instanceof DOMException && e.name === 'AbortError') {
        outcome = 'aborted'; // already reported by the cancel handler
      } else {
        this.failed++;
      }
    } finally {
      this.inflight.delete(t.id);
      // Deliveries complete the attempt implicitly via ingest; aborts were
      // reported when the cancel was honoured; everything else is a failure
      // the engine should know about so the chunk re-pends.
      if (outcome === 'failed') this.map.report_fetch_failed(t.id);
    }
  }
}
