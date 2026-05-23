import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { api } from "../api/client";

/**
 * Browse files already on the incoming volume — rsync drops AND
 * completed TUS uploads — and trigger ingest without re-uploading.
 *
 * Two ways for files to land here:
 *   1. Operator drops them on `<TILESERVER_INCOMING_DIR>/` via
 *      ssh/sftp/rsync. Listed with `source: "rsync"`.
 *   2. Another curator's browser uploaded them via the bulk-upload
 *      screen (TUS). Listed with `source: "upload"`, with the
 *      `upload_id` the bulk endpoint takes as input.
 *
 * Curators forget which mechanism they used last week. The unified
 * listing here makes "ingest that file from yesterday" a one-click
 * action.
 */

type IncomingFile =
  | {
      source: "rsync";
      name: string;
      size_bytes: number;
    }
  | {
      source: "upload";
      name: string;
      size_bytes: number;
      total_bytes: number;
      complete: boolean;
      upload_id: string;
      created_at: string;
    };

interface IncomingListResponse {
  incoming_dir: string;
  files: IncomingFile[];
}

const JOBS = [{ id: "dtm-load", label: "DTM raster load (.tif)" }];

export function Incoming() {
  const nav = useNavigate();
  const qc = useQueryClient();
  const list = useQuery({
    queryKey: ["incoming"],
    queryFn: () => api.get<IncomingListResponse>("/ingest/incoming"),
    refetchInterval: 15_000,
  });
  const trigger = useMutation({
    mutationFn: ({
      job,
      file,
      uploadId,
      source,
    }: {
      job: string;
      file?: string;
      uploadId?: string;
      source: string;
    }) =>
      api.post<{ ok: boolean; job: string; run_id: string }>("/ingest/bulk", {
        job,
        ...(file ? { file } : {}),
        ...(uploadId ? { upload_id: uploadId } : {}),
        source,
      }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["ingest-jobs"] });
      nav("/jobs");
    },
  });

  const [job, setJob] = useState("dtm-load");
  const [source, setSource] = useState("dtm10");

  const files = list.data?.files ?? [];

  return (
    <div className="p-8 max-w-5xl">
      <header className="mb-6">
        <h1 className="text-2xl font-semibold">Incoming files</h1>
        <p className="text-ink-500 text-sm mt-1">
          Files already on the server, from either rsync drops or
          completed browser uploads. Pick one and trigger ingest
          without re-uploading. Auto-refreshes every 15 s.
        </p>
        {list.data && (
          <p className="text-ink-400 text-xs mt-2 font-mono">
            {list.data.incoming_dir}
          </p>
        )}
      </header>

      <div className="flex gap-3 mb-4 items-end">
        <label className="block flex-1">
          <span className="text-sm font-medium text-ink-700">Job</span>
          <select
            value={job}
            onChange={(e) => setJob(e.target.value)}
            className={inputClass}
          >
            {JOBS.map((j) => (
              <option key={j.id} value={j.id}>
                {j.label}
              </option>
            ))}
          </select>
        </label>
        <label className="block flex-1">
          <span className="text-sm font-medium text-ink-700">Source label</span>
          <input
            className={inputClass}
            value={source}
            onChange={(e) => setSource(e.target.value)}
            placeholder="dtm10"
          />
        </label>
      </div>

      <div
        className="rounded border border-ink-200 bg-white overflow-hidden"
        data-testid="incoming-table"
      >
        <table className="w-full text-sm">
          <thead className="bg-ink-100 text-ink-700">
            <tr>
              <th className="text-left px-3 py-2 font-medium">Name</th>
              <th className="text-left px-3 py-2 font-medium">Origin</th>
              <th className="text-right px-3 py-2 font-medium">Size</th>
              <th className="text-left px-3 py-2 font-medium">Status</th>
              <th className="px-3 py-2"></th>
            </tr>
          </thead>
          <tbody>
            {list.isLoading && (
              <tr>
                <td colSpan={5} className="px-3 py-4 text-ink-500">
                  Loading…
                </td>
              </tr>
            )}
            {files.length === 0 && !list.isLoading && (
              <tr>
                <td colSpan={5} className="px-3 py-4 text-ink-500">
                  Nothing here. Drop a file under{" "}
                  <code>{list.data?.incoming_dir}</code> or use the bulk
                  upload screen.
                </td>
              </tr>
            )}
            {files.map((f, i) => {
              const isUpload = f.source === "upload";
              const usable = !isUpload || f.complete;
              return (
                <tr
                  key={i}
                  className="border-t border-ink-200"
                  data-testid={`incoming-row-${f.source}-${f.name}`}
                >
                  <td className="px-3 py-2 font-mono text-xs">{f.name}</td>
                  <td className="px-3 py-2 text-ink-500 text-xs">
                    {f.source}
                  </td>
                  <td className="px-3 py-2 text-right tabular-nums">
                    {formatBytes(f.size_bytes)}
                    {isUpload && !f.complete && (
                      <span className="text-amber-700 ml-1">
                        / {formatBytes(f.total_bytes)}
                      </span>
                    )}
                  </td>
                  <td className="px-3 py-2 text-xs">
                    {isUpload ? (
                      f.complete ? (
                        <span className="text-emerald-700">complete</span>
                      ) : (
                        <span className="text-amber-700">incomplete</span>
                      )
                    ) : (
                      <span className="text-ink-500">on disk</span>
                    )}
                  </td>
                  <td className="px-3 py-2 text-right">
                    <button
                      type="button"
                      disabled={!usable || trigger.isPending}
                      onClick={() =>
                        trigger.mutate({
                          job,
                          source,
                          file: isUpload ? undefined : f.name,
                          uploadId: isUpload ? f.upload_id : undefined,
                        })
                      }
                      className="text-xs px-3 py-1 rounded bg-ink-900 text-ink-50 hover:bg-ink-700 disabled:opacity-50"
                      data-testid={`incoming-ingest-${f.source}-${f.name}`}
                    >
                      Ingest
                    </button>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>

      {trigger.error && (
        <div className="mt-4 text-sm text-red-700">
          {(trigger.error as Error).message}
        </div>
      )}
    </div>
  );
}

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / (1024 * 1024)).toFixed(1)} MB`;
  return `${(n / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

const inputClass =
  "w-full px-3 py-2 text-sm border border-ink-200 rounded bg-white focus:outline-none focus:ring-2 focus:ring-ink-700";
