import { useIngestJobs } from "../api/queries";

const STATUS_CLASS: Record<string, string> = {
  queued: "bg-ink-200 text-ink-700",
  running: "bg-amber-100 text-amber-800",
  succeeded: "bg-emerald-100 text-emerald-800",
  failed: "bg-red-100 text-red-800",
};

export function Jobs() {
  const jobs = useIngestJobs();
  const rows = jobs.data?.rows ?? [];

  return (
    <div className="p-8 max-w-6xl">
      <header className="mb-6">
        <h1 className="text-2xl font-semibold">Ingest jobs</h1>
        <p className="text-ink-500 text-sm mt-1">
          Live status of FKB, Turbase, DNT, and DTM10 imports. Auto-refreshes
          every 3 seconds while any job is running.
        </p>
      </header>
      <div className="rounded border border-ink-200 bg-white overflow-hidden">
        <table className="w-full text-sm">
          <thead className="bg-ink-100 text-ink-700">
            <tr>
              <th className="text-left px-3 py-2 font-medium">Job</th>
              <th className="text-left px-3 py-2 font-medium">Status</th>
              <th className="text-right px-3 py-2 font-medium">Rows in</th>
              <th className="text-right px-3 py-2 font-medium">Upserted</th>
              <th className="text-left px-3 py-2 font-medium">Started</th>
              <th className="text-left px-3 py-2 font-medium">Finished</th>
              <th className="text-left px-3 py-2 font-medium">Error</th>
            </tr>
          </thead>
          <tbody>
            {rows.length === 0 && (
              <tr>
                <td colSpan={7} className="px-3 py-4 text-ink-500">
                  No jobs run yet. Trigger one from the dashboard.
                </td>
              </tr>
            )}
            {rows.map((j) => (
              <tr key={j.id} className="border-t border-ink-200">
                <td className="px-3 py-2 font-medium">{j.name}</td>
                <td className="px-3 py-2">
                  <span
                    className={`text-xs px-2 py-0.5 rounded ${STATUS_CLASS[j.status] ?? ""}`}
                  >
                    {j.status}
                  </span>
                </td>
                <td className="px-3 py-2 text-right tabular-nums">
                  {j.rows_in}
                </td>
                <td className="px-3 py-2 text-right tabular-nums">
                  {j.rows_upserted}
                </td>
                <td className="px-3 py-2 text-ink-500">
                  {j.started_at
                    ? new Date(j.started_at).toLocaleString()
                    : "—"}
                </td>
                <td className="px-3 py-2 text-ink-500">
                  {j.finished_at
                    ? new Date(j.finished_at).toLocaleString()
                    : "—"}
                </td>
                <td className="px-3 py-2 text-red-700 text-xs">
                  {j.error_text ?? ""}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
