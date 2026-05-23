import { Link } from "react-router-dom";
import { useSummary, useIngestJobs, useTriggerIngest } from "../api/queries";
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
  { id: "dtm10-attach", label: "DTM10 elevation" },
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
    </div>
  );
}
