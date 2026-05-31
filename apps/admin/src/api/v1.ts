/**
 * Client for the public /v1/* API (primitive surface + per-primitive
 * debug). Separate from `./client.ts` which targets /admin/api/*.
 *
 * Debug endpoints are gated server-side by `TURBO_ENABLE_DEBUG=1`
 * once Stage 7 lands; until then they're always available.
 */

import { ApiError } from "./client";

const V1_BASE = "/v1";

async function request<T>(
  path: string,
  init: RequestInit = {},
): Promise<T> {
  const headers = new Headers(init.headers ?? {});
  if (!headers.has("Accept")) headers.set("Accept", "application/json");
  if (init.body && !headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json");
  }
  const res = await fetch(`${V1_BASE}${path}`, {
    credentials: "include",
    ...init,
    headers,
  });
  if (!res.ok) {
    let body: unknown;
    try {
      body = await res.json();
    } catch {
      body = await res.text();
    }
    throw new ApiError(res.status, body);
  }
  if (res.status === 204) return undefined as T;
  return (await res.json()) as T;
}

export const v1 = {
  get: <T,>(path: string) => request<T>(path),
  post: <T,>(path: string, body: unknown) =>
    request<T>(path, { method: "POST", body: JSON.stringify(body) }),
};

// --- Stage 1: elevation primitive ---

export interface ElevSampleResp {
  lon: number;
  lat: number;
  x_25833: number;
  y_25833: number;
  elev_m: number | null;
  took_us: number;
}

export interface ElevProfileResp {
  elev_m: (number | null)[];
  distances_m: number[];
  took_us: number;
}

export interface DemCoverage {
  min_x: number;
  min_y: number;
  max_x: number;
  max_y: number;
  cells_x: number;
  cells_y: number;
  resolution_m: number;
  nodata: number;
  tile_cells: number;
  tiles_x: number;
  tiles_y: number;
  tiles_present: number;
  tiles_absent: number;
  file_size_bytes: number;
  build_timestamp_unix_sec: number;
  cache: {
    entries: number;
    total_bytes: number;
    capacity_bytes: number;
    hits: number;
    misses: number;
    evictions: number;
  };
}

export interface ElevBenchResp {
  sample_p50_us: number;
  sample_p99_us: number;
  sample_mean_us: number;
  sample_count: number;
  profile_p50_us: number;
  profile_p99_us: number;
  profile_mean_us: number;
  profile_count: number;
  profile_points: number;
}

// --- Stage 2: slope + aspect ---

export interface SlopeSampleResp {
  lon: number;
  lat: number;
  slope_deg: number | null;
  aspect_deg: number | null;
  took_us: number;
}

export interface SlopeAlongStep {
  distance_m: number;
  slope_deg: number | null;
  aspect_deg: number | null;
}

export interface SlopeAlongResp {
  steps: SlopeAlongStep[];
  took_us: number;
}

// --- Stage 3: refusal mask ---

export interface MaskSampleResp {
  lon: number;
  lat: number;
  refused: boolean;
  kind: "none" | "water" | "glacier" | "reserved3";
  took_us: number;
}

export interface MaskCoverage {
  meta: {
    min_x: number;
    min_y: number;
    max_x: number;
    max_y: number;
    cells_x: number;
    cells_y: number;
    resolution_m: number;
  };
  file_size_bytes: number;
  cells_total: number;
  cells_water: number;
  cells_glacier: number;
}

// --- Stage 4: routing graph ---

export type Profile = "foot" | "bicycle" | "ski";

export interface RouteResp {
  from_node: number;
  to_node: number;
  length_m: number;
  cost: number;
  geometry: [number, number][];
  edges: number[];
  took_us: number;
}

export interface GraphStats {
  meta: {
    node_count: number;
    edge_count: number;
    profile_count: number;
    srid: number;
  };
  file_size_bytes: number;
  avg_edges_per_node: number;
  min_x: number;
  min_y: number;
  max_x: number;
  max_y: number;
}

export interface GraphDensity {
  points: [number, number][];
  source_count: number;
  returned_count: number;
}

export interface InspectCell {
  lon: number;
  lat: number;
  cost_mul: number;
}

export interface InspectResp {
  inspect: {
    mesh_cell_m: number;
    cells: InspectCell[];
    refused_polygons: [number, number][][];
    refused_by: string[];
    nearest_graph_node_from: [number, number] | null;
    nearest_graph_node_to: [number, number] | null;
  };
  took_us: number;
}

export interface CellInspectLayer {
  name: string;
  multiplier: number;
  refused: string | null;
  covers: boolean;
}

export interface CellInspectResp {
  point: {
    lon: number;
    lat: number;
    x_25833: number;
    y_25833: number;
    composed_multiplier: number;
    refused_by: string | null;
    layers: CellInspectLayer[];
  };
  took_us: number;
}

// --- Viewport-bbox data inspectors -----------------------------------

export interface MaskCellsResp {
  cells: [number, number, number][]; // [lon, lat, value]
  resolution_m: number;
  returned: number;
  bbox_clipped: boolean;
}

export interface EdgePolyline {
  /** Full vertex sequence from graph_geom (lon, lat pairs). When
   * the backend has no graph_geom attached this is a 2-point
   * straight line between endpoint nodes and `has_polylines` will
   * be false. */
  coords: [number, number][];
  kind: number;
}

export interface EdgesResp {
  edges: EdgePolyline[];
  returned: number;
  capped: boolean;
  has_polylines: boolean;
}

export interface AnchorsBboxResp {
  anchors: {
    id: number;
    lon: number;
    lat: number;
    kind: string;
    name: string | null;
    elev_m: number;
  }[];
  returned: number;
  capped: boolean;
}

// --- Stage 5: anchor search ---

export type AnchorKind =
  | "unknown"
  | "summit"
  | "cabin"
  | "viewpoint"
  | "trailhead"
  | "parking"
  | "waterfeature"
  | "named_place";

export interface AnchorHit {
  id: number;
  kind: AnchorKind;
  name: string | null;
  x: number;
  y: number;
  elev_m: number;
  distance_m: number;
}

export interface NearestResp {
  anchors: AnchorHit[];
  took_us: number;
}
export interface NameResp {
  anchors: AnchorHit[];
  took_us: number;
}
export interface SearchCoverage {
  meta: { count: number; names_size: number };
  file_size_bytes: number;
  by_kind: Record<string, number>;
}

// --- Stage 6: off-trail pathfind ---

export type PathStrategy = "on_graph" | "off_trail" | "hybrid";

export type LegKind = "off_trail_prefix" | "graph" | "off_trail_suffix";

export interface PathLeg {
  kind: LegKind;
  start_idx: number;
  end_idx: number;
  length_m: number;
}

export type SolverEvent =
  | { kind: "mesh_built"; cells: number; refused_cells: number }
  | { kind: "node_popped"; x: number; y: number; g: number; h: number }
  | {
      kind: "edge_relaxed";
      fx: number;
      fy: number;
      tx: number;
      ty: number;
      new_g: number;
      took_los: boolean;
    }
  | {
      kind: "line_of_sight_cast";
      fx: number;
      fy: number;
      tx: number;
      ty: number;
      blocked: boolean;
    }
  | { kind: "best_path_snapshot"; coords: [number, number][] };

export interface PhaseFrame {
  name: string;
  started_at_us: number;
  events: SolverEvent[];
}

export interface SolverRecording {
  phases: PhaseFrame[];
  decimated: boolean;
  events_observed: number;
  events_retained: number;
}

export interface PathfindResp {
  path: {
    strategy: PathStrategy;
    geometry: [number, number][];
    distances_m: number[];
    length_m: number;
    cost: number;
    on_trail_pct: number;
    /** Metres of route by surface type. Keys: sti, vei, traktorvei,
     * skogsvei, skiloype, off_trail, unknown. Empty for trivial
     * paths or older servers. */
    fkb_breakdown?: Record<string, number>;
    legs: PathLeg[];
    refused_by: string[];
    /** Per-event solver recording — present when posting to
     * /v1/pathfind/record (or when Prefs.record=true). The SPA's
     * Algorithm-replay panel animates it. */
    recording?: SolverRecording;
  };
  took_us: number;
  layers: string[];
}

export interface LayersResp {
  layers: string[];
}
