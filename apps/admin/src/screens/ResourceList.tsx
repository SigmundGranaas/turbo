import { useState } from "react";
import { Link, useParams } from "react-router-dom";
import { useResourceList, useArchiveRoute } from "../api/queries";
import type { Resource } from "../api/types";

const RESOURCE_LABEL: Record<string, string> = {
  "hiking-trails": "Hiking trails",
  "ski-tracks": "Ski tracks",
  "forest-roads": "Forest roads",
  "cycling-routes": "Cycling routes",
};

export function ResourceList() {
  const { resource } = useParams<{ resource: Resource }>();
  const [status, setStatus] = useState<string>("");
  const [q, setQ] = useState("");
  const [offset, setOffset] = useState(0);
  const limit = 50;
  const list = useResourceList(resource as Resource, {
    status: status || undefined,
    q: q || undefined,
    limit,
    offset,
  });
  const archive = useArchiveRoute(resource as Resource);

  if (!resource) return null;
  const rows = list.data?.rows ?? [];
  const total = list.data?.total ?? 0;

  return (
    <div className="p-8 max-w-6xl">
      <header className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-semibold">
            {RESOURCE_LABEL[resource] ?? resource}
          </h1>
          <p className="text-ink-500 text-sm mt-1">
            {total} curated route{total === 1 ? "" : "s"}
          </p>
        </div>
        <Link
          to={`/resources/${resource}/new`}
          className="text-sm px-3 py-2 rounded bg-ink-900 text-ink-50 hover:bg-ink-700"
        >
          Create route
        </Link>
      </header>

      <div className="flex gap-2 mb-4">
        <input
          type="text"
          value={q}
          onChange={(e) => {
            setQ(e.target.value);
            setOffset(0);
          }}
          placeholder="Search name…"
          className="px-3 py-2 text-sm border border-ink-200 rounded bg-white flex-1"
        />
        <select
          value={status}
          onChange={(e) => {
            setStatus(e.target.value);
            setOffset(0);
          }}
          className="px-3 py-2 text-sm border border-ink-200 rounded bg-white"
        >
          <option value="">All status</option>
          <option value="draft">Draft</option>
          <option value="published">Published</option>
          <option value="archived">Archived</option>
        </select>
      </div>

      <div className="rounded border border-ink-200 bg-white overflow-hidden">
        <table className="w-full text-sm">
          <thead className="bg-ink-100 text-ink-700">
            <tr>
              <th className="text-left px-3 py-2 font-medium">Name</th>
              <th className="text-left px-3 py-2 font-medium">Status</th>
              <th className="text-left px-3 py-2 font-medium">Source</th>
              <th className="text-right px-3 py-2 font-medium">Length</th>
              <th className="text-left px-3 py-2 font-medium">Updated</th>
              <th className="px-3 py-2"></th>
            </tr>
          </thead>
          <tbody>
            {list.isLoading && (
              <tr>
                <td colSpan={6} className="px-3 py-4 text-ink-500">
                  Loading…
                </td>
              </tr>
            )}
            {rows.length === 0 && !list.isLoading && (
              <tr>
                <td colSpan={6} className="px-3 py-4 text-ink-500">
                  No routes yet. Upload a GPX or create one manually.
                </td>
              </tr>
            )}
            {rows.map((r) => (
              <tr
                key={r.id}
                className="border-t border-ink-200 hover:bg-ink-50"
              >
                <td className="px-3 py-2">
                  <Link
                    to={`/resources/${resource}/${r.id}`}
                    className="text-ink-900 hover:underline"
                  >
                    {r.name ?? r.slug}
                  </Link>
                  {r.needs_review && (
                    <span className="ml-2 text-xs px-1.5 py-0.5 rounded bg-amber-100 text-amber-800">
                      review
                    </span>
                  )}
                </td>
                <td className="px-3 py-2 capitalize">{r.status}</td>
                <td className="px-3 py-2">{r.source}</td>
                <td className="px-3 py-2 text-right tabular-nums">
                  {r.length_m
                    ? `${(r.length_m / 1000).toFixed(1)} km`
                    : "—"}
                </td>
                <td className="px-3 py-2 text-ink-500">
                  {new Date(r.updated_at).toLocaleDateString()}
                </td>
                <td className="px-3 py-2 text-right">
                  <button
                    type="button"
                    onClick={() => {
                      if (confirm(`Archive “${r.name ?? r.slug}”?`)) {
                        archive.mutate(r.id);
                      }
                    }}
                    className="text-xs text-ink-500 hover:text-ink-900"
                  >
                    Archive
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {total > limit && (
        <div className="flex items-center justify-between mt-4 text-sm">
          <button
            type="button"
            onClick={() => setOffset(Math.max(0, offset - limit))}
            disabled={offset === 0}
            className="px-3 py-1.5 rounded border border-ink-200 bg-white disabled:opacity-50"
          >
            Previous
          </button>
          <div className="text-ink-500">
            {offset + 1}–{Math.min(offset + limit, total)} of {total}
          </div>
          <button
            type="button"
            onClick={() => setOffset(offset + limit)}
            disabled={offset + limit >= total}
            className="px-3 py-1.5 rounded border border-ink-200 bg-white disabled:opacity-50"
          >
            Next
          </button>
        </div>
      )}
    </div>
  );
}
