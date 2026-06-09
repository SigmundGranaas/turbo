import { useMemo, useState } from "react";

import {
  useGeonorgeAreas,
  useIngestJobs,
  useProvision,
} from "../api/queries";
import type { IngestJob } from "../api/types";

/**
 * One-click data provisioning. Pick a county (or the whole country), click
 * Provision, and the tileserver orders the N50 dump from Geonorge, downloads
 * it, restores it, and runs every canonical upsert — no file upload, no
 * rsync, no terminal. Progress streams from the `provision-n50` job rows
 * below (the same log the Jobs screen shows).
 */
export function Provision() {
  const areas = useGeonorgeAreas();
  const provision = useProvision();
  const jobs = useIngestJobs();
  const [area, setArea] = useState("");
  const [force, setForce] = useState(false);

  const provisionRuns = useMemo(
    () => (jobs.data?.rows ?? []).filter((r) => r.name === "provision-n50"),
    [jobs.data],
  );

  return (
    <div className="mx-auto max-w-3xl space-y-6 p-6">
      <header>
        <h1 className="text-xl font-semibold">Provision map data</h1>
        <p className="mt-1 text-sm text-gray-600">
          Download and ingest Kartverket N50 directly — the server fetches from
          Geonorge, restores, and upserts every layer. No manual steps.
        </p>
      </header>

      <section className="rounded border border-gray-200 p-4">
        <label className="block text-sm font-medium" htmlFor="area">
          Area
        </label>
        <div className="mt-1 flex flex-wrap items-center gap-3">
          <select
            id="area"
            className="min-w-56 rounded border border-gray-300 px-3 py-2"
            value={area}
            onChange={(e) => setArea(e.target.value)}
            disabled={areas.isLoading || provision.isPending}
          >
            <option value="" disabled>
              {areas.isLoading ? "Loading counties…" : "Select a county…"}
            </option>
            {(areas.data?.areas ?? []).map((a) => (
              <option key={a.code} value={a.code}>
                {a.name}
                {a.type === "fylke" ? ` (${a.code})` : ""}
              </option>
            ))}
          </select>

          <label className="flex items-center gap-2 text-sm text-gray-700">
            <input
              type="checkbox"
              checked={force}
              onChange={(e) => setForce(e.target.checked)}
            />
            Force re-download &amp; restore
          </label>

          <button
            type="button"
            className="rounded bg-blue-600 px-4 py-2 text-white disabled:opacity-50"
            disabled={!area || provision.isPending}
            onClick={() => provision.mutate({ area, force })}
          >
            {provision.isPending ? "Starting…" : "Provision"}
          </button>
        </div>

        {areas.isError && (
          <p className="mt-2 text-sm text-red-600">
            Couldn’t load the county list from Geonorge. You can still type a
            code via the API; check the server’s outbound network policy.
          </p>
        )}
        {provision.isError && (
          <p className="mt-2 text-sm text-red-600">
            {(provision.error as Error).message}
          </p>
        )}
        {provision.isSuccess && (
          <p className="mt-2 text-sm text-green-700">
            Started — run {provision.data.run_id.slice(0, 8)}. Watch progress
            below.
          </p>
        )}
      </section>

      <section>
        <h2 className="text-sm font-semibold text-gray-700">
          Provisioning runs
        </h2>
        <table className="mt-2 w-full text-sm">
          <thead className="text-left text-gray-500">
            <tr>
              <th className="py-1">Status</th>
              <th>Rows</th>
              <th>Started</th>
              <th>Finished</th>
              <th>Error</th>
            </tr>
          </thead>
          <tbody>
            {provisionRuns.length === 0 && (
              <tr>
                <td colSpan={5} className="py-3 text-gray-400">
                  No provisioning runs yet.
                </td>
              </tr>
            )}
            {provisionRuns.map((r: IngestJob) => (
              <tr key={r.id} className="border-t border-gray-100">
                <td className="py-1">
                  <StatusBadge status={r.status} />
                </td>
                <td>{r.rows_upserted}</td>
                <td className="text-gray-500">{fmt(r.started_at)}</td>
                <td className="text-gray-500">{fmt(r.finished_at)}</td>
                <td className="max-w-xs truncate text-red-600">
                  {r.error_text ?? ""}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>
    </div>
  );
}

function StatusBadge({ status }: { status: string }) {
  const cls =
    status === "succeeded"
      ? "bg-green-100 text-green-800"
      : status === "failed"
        ? "bg-red-100 text-red-800"
        : "bg-amber-100 text-amber-800";
  return (
    <span className={`rounded px-2 py-0.5 text-xs font-medium ${cls}`}>
      {status}
    </span>
  );
}

function fmt(ts: string | null | undefined): string {
  if (!ts) return "—";
  return new Date(ts).toLocaleString();
}
