/** Mirrors the Rust `Resource` enum in turbo-tiles-core. */
export const RESOURCES = [
  "hiking-trails",
  "ski-tracks",
  "forest-roads",
  "cycling-routes",
] as const;
export type Resource = (typeof RESOURCES)[number];

export type RouteStatus = "draft" | "published" | "archived";

export interface RouteRow {
  id: string;
  resource: Resource;
  slug: string;
  name: string | null;
  difficulty: string | null;
  length_m: number | null;
  status: RouteStatus;
  source: string;
  needs_review: boolean;
  updated_at: string;
}

export interface ListResponse {
  rows: RouteRow[];
  total: number;
  limit: number;
  offset: number;
}

export interface RouteDetail {
  id: string;
  resource: Resource;
  slug: string;
  name: string | null;
  description: string | null;
  difficulty: string | null;
  marking: string | null;
  season: string[];
  surface: string | null;
  attribution: string | null;
  length_m: number | null;
  elevation_gain_m: number | null;
  elevation_loss_m: number | null;
  status: RouteStatus;
  source: string;
  external_id: string | null;
  needs_review: boolean;
  created_at: string;
  updated_at: string;
  geometry: GeoJSON.Geometry;
}

export interface SummaryResponse {
  resources: Record<string, Record<RouteStatus, number>>;
}

export interface IngestJob {
  id: number;
  run_id: string;
  name: string;
  status: "queued" | "running" | "succeeded" | "failed";
  started_at: string | null;
  finished_at: string | null;
  rows_in: number;
  rows_upserted: number;
  error_text: string | null;
}

export interface UpdateBody {
  name?: string;
  description?: string;
  difficulty?: string;
  marking?: string;
  season?: string[];
  surface?: string;
  status?: RouteStatus;
  attribution?: string;
}

export interface CreateBody {
  slug: string;
  name?: string;
  description?: string;
  difficulty?: string;
  marking?: string;
  season?: string[];
  surface?: string;
  geometry: GeoJSON.Geometry;
  attribution?: string;
}
