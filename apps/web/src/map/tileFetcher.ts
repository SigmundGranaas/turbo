import type { TurboMap } from 'turbomap-web';
import { type TileKind, type Templates, tileUrl } from './templates';

interface PendingTile {
  kind: TileKind;
  layer: string;
  z: number;
  x: number;
  y: number;
}

/** Drives the host side of the engine's host-driven tile IO: read
 *  `pending_tiles()`, fetch each missing tile via `fetch()`, and push the bytes
 *  back through the matching `ingest_*`. The browser analogue of the Kotlin
 *  `launchTileFetch` reconciler — concurrency-capped, dedup'd by in-flight key.
 *
 *  Stateless on disk for now (online-first); a Cache/IndexedDB read-through
 *  layer slots in here in the offline phase without touching the engine. */
export class TileLoader {
  private inflight = new Set<string>();
  stored = 0;
  failed = 0;

  constructor(
    private readonly map: TurboMap,
    private templates: Templates,
    private readonly maxConcurrent = 24,
  ) {}

  /** Swap the URL templates (e.g. on a base-layer change). Subsequent
   *  `pending_tiles()` for `raster/basemap` then fetch the new source. */
  setTemplates(t: Templates): void {
    this.templates = t;
  }

  /** Kick off fetches for tiles the engine wants and isn't already loading.
   *  Cheap to call every frame — it no-ops once everything is in flight. */
  pump(): void {
    let pending: PendingTile[];
    try {
      pending = JSON.parse(this.map.pending_tiles()) as PendingTile[];
    } catch {
      return;
    }
    for (const t of pending) {
      if (this.inflight.size >= this.maxConcurrent) break;
      const key = `${t.kind}/${t.layer}/${t.z}/${t.x}/${t.y}`;
      if (this.inflight.has(key)) continue;
      const template = this.templates[t.kind]?.[t.layer];
      if (!template) continue;
      this.inflight.add(key);
      void this.fetchOne(t, tileUrl(template, t.z, t.x, t.y), key);
    }
  }

  private async fetchOne(t: PendingTile, url: string, key: string): Promise<void> {
    try {
      const res = await fetch(url);
      // 204/404 = the server has no tile here (e.g. ocean, out of coverage).
      // That's "absent", not a failure — leave it; don't retry-storm.
      if (!res.ok) {
        this.failed++;
        return;
      }
      const bytes = new Uint8Array(await res.arrayBuffer());
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
    } catch {
      this.failed++;
    } finally {
      this.inflight.delete(key);
    }
  }
}
