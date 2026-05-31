import { Link } from "react-router-dom";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useSummary, useIngestJobs, useTriggerIngest } from "../api/queries";
import { api } from "../api/client";
import { RESOURCES } from "../api/types";

const RESOURCE_LABEL: Record<string, string> = {
  "hiking-trails": "Hiking trails",
  "ski-tracks": "Ski tracks",
  "forest-roads": "Forest roads",
  "cycling-routes": "Cycling routes",
};

const JOBS = [
  { id: "fkb-sti", label: "FKB Sti (paths)" },
  { id: "turbase", label: "Nasjonal Turbase" },
  { id: "dnt", label: "DNT" },
  { id: "dtm10-attach", label: "DTM10 elevation (sample-along-edge)" },
  { id: "n50-anchors", label: "N50 anchors (summits + named places, fixture in dev)" },
  { id: "edge-attrs", label: "Edge slope/aspect (from paths.dem)" },
  { id: "recommend-seed", label: "Recommend dev fixture (Oslomarka)" },
  { id: "skeleton-build", label: "Off-trail skeleton (Delaunay over anchors)" },
  { id: "n50-restore", label: "N50: psql-restore dump (file, heavy ~15 min)" },
  { id: "n50-vann-upsert", label: "N50 → terrain.water_polygon" },
  { id: "n50-isogbre-upsert", label: "N50 → terrain.glacier_polygon" },
  { id: "n50-landcover-upsert", label: "N50 → terrain.landcover_patch (skog/myr/...)" },
  { id: "n50-stedsnavn-upsert", label: "N50 → anchors.anchor (stedsnavn + terrengpunkt)" },
  { id: "n50-vegnett-upsert", label: "N50 → paths.edge (roads/tractor)" },
  { id: "turbase-restore", label: "Turrutebasen: psql-restore dump (file)" },
  { id: "turbase-upsert", label: "Turrutebasen → paths.edge + trails.trail" },
  { id: "dnt-cabins-load", label: "DNT cabins (API or file)" },
];

export function Dashboard() {
  const summary = useSummary();
  const jobs = useIngestJobs();
  const trigger = useTriggerIngest();

  return (
    <div className="p-8 space-y-8 max-w-5xl">
      <header>
        <h1 className="text-2xl font-semibold">Dashboard</h1>
        <p className="text-ink-500 text-sm mt-1">
          Curated path counts and ingest control.
        </p>
      </header>

      <section>
        <h2 className="text-sm font-medium uppercase tracking-wide text-ink-500 mb-3">
          Resources
        </h2>
        <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
          {RESOURCES.map((r) => {
            const counts: Record<string, number> =
              summary.data?.resources?.[r] ?? {};
            const total = Object.values(counts).reduce(
              (a, b) => a + (b ?? 0),
              0,
            );
            return (
              <Link
                key={r}
                to={`/resources/${r}`}
                className="block p-4 rounded border border-ink-200 bg-white hover:border-ink-400 transition-colors"
              >
                <div className="text-sm text-ink-500">
                  {RESOURCE_LABEL[r]}
                </div>
                <div className="text-2xl font-semibold mt-1">{total}</div>
                <div className="text-xs text-ink-500 mt-2">
                  {Object.entries(counts)
                    .map(([k, v]) => `${k}: ${v}`)
                    .join(" · ") || "no curated routes"}
                </div>
              </Link>
            );
          })}
        </div>
      </section>

      <section>
        <h2 className="text-sm font-medium uppercase tracking-wide text-ink-500 mb-3">
          Ingest jobs
        </h2>
        <div className="space-y-2">
          {JOBS.map((j) => {
            const lastRun = jobs.data?.rows?.find((row) => row.name === j.id);
            return (
              <div
                key={j.id}
                className="flex items-center justify-between p-3 rounded border border-ink-200 bg-white"
              >
                <div>
                  <div className="font-medium text-sm">{j.label}</div>
                  <div className="text-xs text-ink-500 mt-1">
                    {lastRun
                      ? `Last run: ${lastRun.status} · ${lastRun.rows_upserted} upserted${lastRun.finished_at ? ` · ${new Date(lastRun.finished_at).toLocaleString()}` : ""}`
                      : "Never run"}
                  </div>
                </div>
                <button
                  type="button"
                  onClick={() => trigger.mutate(j.id)}
                  disabled={trigger.isPending}
                  className="text-sm px-3 py-1.5 rounded bg-ink-900 text-ink-50 hover:bg-ink-700 disabled:opacity-50"
                >
                  Trigger
                </button>
              </div>
            );
          })}
        </div>
      </section>

      <ResetSection />
    </div>
  );
}

interface StateSummary {
  paths: { nodes: number; edges: number; skeleton_edges: number; dem_tiles: number };
  anchors: { total: number; snapped: number };
  terrain: { water_polygons: number; glacier_polygons: number; landcover_patches: number };
  trails: { total: number };
  staging: { n50_present: boolean; turbase_present: boolean };
  recommend: { attr_version: number };
}

const RESET_SCOPES: Array<{ id: string; label: string; danger?: boolean }> = [
  { id: "recommend", label: "Reset dev fixture (anchors + sample water/trail)" },
  { id: "skeleton", label: "Reset skeleton edges only" },
  { id: "n50_staging", label: "Drop n50_staging schema" },
  { id: "turbase_staging", label: "Drop turbase_staging schema" },
  { id: "canonical", label: "Reset canonical (terrain.* + anchors.* + trails.*)" },
  { id: "all", label: "RESET ALL DATA (paths.node/edge/dem too)", danger: true },
];

function ResetSection() {
  const qc = useQueryClient();
  const state = useQuery({
    queryKey: ["db-state"],
    queryFn: () => api.get<StateSummary>("/state"),
    refetchInterval: 5000,
  });
  const reset = useMutation({
    mutationFn: (scope: string) =>
      api.post<{ ok: boolean; scope: string; actions: string[] }>(
        `/reset/${scope}`,
        {},
      ),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["db-state"] });
      qc.invalidateQueries({ queryKey: ["ingest-jobs"] });
      qc.invalidateQueries({ queryKey: ["summary"] });
    },
  });

  const onReset = (scope: string, danger?: boolean) => {
    if (danger && !window.confirm(`Really ${scope}? This is destructive.`)) return;
    reset.mutate(scope);
  };

  const s = state.data;
  return (
    <section>
      <h2 className="text-sm font-medium uppercase tracking-wide text-ink-500 mb-3">
        Database state + reset
      </h2>
      {s && (
        <div className="grid grid-cols-3 md:grid-cols-6 gap-3 text-xs mb-3 p-3 rounded border border-ink-200 bg-white">
          <StatBox label="Nodes" v={s.paths.nodes} />
          <StatBox label="Edges" v={s.paths.edges} />
          <StatBox label="Skeleton" v={s.paths.skeleton_edges} />
          <StatBox label="Anchors" v={`${s.anchors.snapped}/${s.anchors.total}`} />
          <StatBox label="Trails" v={s.trails.total} />
          <StatBox label="DEM tiles" v={s.paths.dem_tiles} />
          <StatBox label="Lakes" v={s.terrain.water_polygons} />
          <StatBox label="Glaciers" v={s.terrain.glacier_polygons} />
          <StatBox label="Landcover" v={s.terrain.landcover_patches} />
          <StatBox label="n50_staging" v={s.staging.n50_present ? "✓" : "—"} />
          <StatBox label="turbase_stg" v={s.staging.turbase_present ? "✓" : "—"} />
          <StatBox label="attr_version" v={s.recommend.attr_version} />
        </div>
      )}
      <div className="space-y-2">
        {RESET_SCOPES.map((r) => (
          <div
            key={r.id}
            className={`flex items-center justify-between p-3 rounded border ${
              r.danger
                ? "border-red-300 bg-red-50"
                : "border-ink-200 bg-white"
            }`}
          >
            <div className="font-medium text-sm">{r.label}</div>
            <button
              type="button"
              onClick={() => onReset(r.id, r.danger)}
              disabled={reset.isPending}
              className={`text-sm px-3 py-1.5 rounded text-ink-50 disabled:opacity-50 ${
                r.danger ? "bg-red-700 hover:bg-red-600" : "bg-ink-900 hover:bg-ink-700"
              }`}
            >
              {reset.isPending && reset.variables === r.id ? "Resetting…" : "Reset"}
            </button>
          </div>
        ))}
      </div>
      {reset.data && (
        <div className="mt-3 p-2 bg-emerald-50 border border-emerald-300 rounded text-xs">
          <div className="font-medium">Last reset ({reset.data.scope}):</div>
          <ul className="mt-1 list-disc pl-4">
            {reset.data.actions.map((a, i) => (
              <li key={i}>{a}</li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}

function StatBox({ label, v }: { label: string; v: number | string }) {
  return (
    <div>
      <div className="text-ink-500">{label}</div>
      <div className="font-medium tabular-nums">{v}</div>
    </div>
  );
}
