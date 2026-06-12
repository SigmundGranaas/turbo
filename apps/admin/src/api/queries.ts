import {
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { api } from "./client";
import type {
  CreateBody,
  IngestJob,
  ListResponse,
  Resource,
  RouteDetail,
  SummaryResponse,
  UpdateBody,
} from "./types";

export function useSummary() {
  return useQuery({
    queryKey: ["summary"],
    queryFn: () => api.get<SummaryResponse>("/resources"),
  });
}

interface ListParams {
  status?: string;
  source?: string;
  q?: string;
  limit?: number;
  offset?: number;
}

function listQs(p: ListParams): string {
  const u = new URLSearchParams();
  if (p.status) u.set("status", p.status);
  if (p.source) u.set("source", p.source);
  if (p.q) u.set("q", p.q);
  if (p.limit) u.set("limit", String(p.limit));
  if (p.offset) u.set("offset", String(p.offset));
  return u.toString() ? `?${u.toString()}` : "";
}

export function useResourceList(resource: Resource, params: ListParams = {}) {
  return useQuery({
    queryKey: ["resource-list", resource, params],
    queryFn: () =>
      api.get<ListResponse>(`/resources/${resource}${listQs(params)}`),
  });
}

export function useRouteDetail(resource: Resource, id: string) {
  return useQuery({
    queryKey: ["route-detail", resource, id],
    queryFn: () => api.get<RouteDetail>(`/resources/${resource}/${id}`),
    enabled: Boolean(id),
  });
}

export function useUpdateRoute(resource: Resource, id: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: UpdateBody) =>
      api.put<{ ok: boolean }>(`/resources/${resource}/${id}`, body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["resource-list", resource] });
      qc.invalidateQueries({ queryKey: ["route-detail", resource, id] });
      qc.invalidateQueries({ queryKey: ["summary"] });
    },
  });
}

export function useCreateRoute(resource: Resource) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: CreateBody) =>
      api.post<{ id: string }>(`/resources/${resource}`, body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["resource-list", resource] });
      qc.invalidateQueries({ queryKey: ["summary"] });
    },
  });
}

export function useArchiveRoute(resource: Resource) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) =>
      api.del<{ ok: boolean }>(`/resources/${resource}/${id}`),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["resource-list", resource] });
      qc.invalidateQueries({ queryKey: ["summary"] });
    },
  });
}

export function useUploadGpx() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (form: FormData) =>
      api.postForm<{ id: string; resource: Resource; slug: string; name: string }>(
        "/upload-gpx",
        form,
      ),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["resource-list"] });
      qc.invalidateQueries({ queryKey: ["summary"] });
    },
  });
}

export function useIngestJobs() {
  return useQuery({
    queryKey: ["ingest-jobs"],
    queryFn: () => api.get<{ rows: IngestJob[] }>("/ingest/jobs"),
    refetchInterval: (q) => {
      // Poll while any job is in flight.
      const rows = q.state.data?.rows ?? [];
      return rows.some((r) => r.status === "running" || r.status === "queued")
        ? 3_000
        : false;
    },
  });
}

export function useTriggerIngest() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (job: string) =>
      api.post<{ ok: boolean; job: string; run_id: string }>(
        `/ingest/${job}/trigger`,
        {},
      ),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["ingest-jobs"] });
    },
  });
}

export interface GeonorgeArea {
  code: string;
  name: string;
  type: string;
}

/** Counties + "Whole country" for the Provision picker (proxied codelist). */
export function useGeonorgeAreas() {
  return useQuery({
    queryKey: ["geonorge-areas"],
    queryFn: () => api.get<{ areas: GeonorgeArea[] }>("/geonorge/areas"),
    staleTime: 60 * 60 * 1000, // codelist is effectively static
  });
}

/**
 * Kick off the fully-automated N50 provisioning chain (Geonorge download →
 * restore → all upserts) for an area. Returns a run_id; the Jobs view (and
 * the inline log) follow `provision-n50` to completion.
 */
export function useProvision() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: { area: string; force?: boolean }) =>
      api.post<{ ok: boolean; job: string; area: string; run_id: string }>(
        "/provision",
        body,
      ),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["ingest-jobs"] });
    },
  });
}
