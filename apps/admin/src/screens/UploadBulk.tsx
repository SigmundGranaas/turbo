import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Upload } from "tus-js-client";
import { useQueryClient } from "@tanstack/react-query";

import { api } from "../api/client";
import { RESOURCES, type Resource } from "../api/types";

/**
 * Resumable upload of a multi-GB Geonorge dataset over TUS, then
 * trigger an ingest job referencing the upload by id. The browser
 * sends 5 MB chunks via PATCH; if the network blips or the laptop
 * sleeps, tus-js-client resumes from the last acknowledged offset on
 * the next attempt.
 *
 * The TUS endpoint lives at `/admin/api/upload` on the same origin
 * the SPA was served from, so the access_token cookie flows through
 * the YARP gateway automatically.
 */

type Status =
  | { kind: "idle" }
  | { kind: "uploading"; uploadedBytes: number; totalBytes: number }
  | { kind: "paused"; uploadedBytes: number; totalBytes: number }
  | { kind: "uploaded"; uploadId: string; filename: string }
  | { kind: "ingesting"; uploadId: string }
  | { kind: "done"; jobName: string }
  | { kind: "error"; message: string };

const JOBS: { id: string; label: string }[] = [
  { id: "dtm-load", label: "DTM raster load (.tif)" },
];

export function UploadBulk() {
  const nav = useNavigate();
  const qc = useQueryClient();
  const [file, setFile] = useState<File | null>(null);
  const [resource, setResource] = useState<Resource>("hiking-trails");
  const [job, setJob] = useState<string>("dtm-load");
  const [source, setSource] = useState<string>("dtm10");
  const [status, setStatus] = useState<Status>({ kind: "idle" });
  const uploadRef = useRef<Upload | null>(null);

  // Abort any in-flight upload when the curator navigates away.
  // tus-js-client returns a Promise from abort() but useEffect's
  // cleanup wants a void/Destructor, so wrap in a fire-and-forget.
  useEffect(
    () => () => {
      void uploadRef.current?.abort(true);
    },
    [],
  );

  const onStart = () => {
    if (!file) return;
    setStatus({
      kind: "uploading",
      uploadedBytes: 0,
      totalBytes: file.size,
    });
    const upload = new Upload(file, {
      endpoint: "/admin/api/upload",
      // Cookie auth flows through fetch's default credentials, but
      // tus-js-client uses XHR which doesn't include cookies cross-
      // origin by default. We're same-origin so this is implicit; the
      // override is left here as a hook for future cross-origin
      // deployments.
      retryDelays: [0, 1_000, 3_000, 5_000, 10_000, 30_000],
      // 5 MB chunks — the server caps a single PATCH at 16 MB and the
      // YARP gateway has its body limit lifted accordingly.
      chunkSize: 5 * 1024 * 1024,
      metadata: { filename: file.name },
      onError: (err) =>
        setStatus({ kind: "error", message: err.message ?? String(err) }),
      onProgress: (uploadedBytes, totalBytes) =>
        setStatus({ kind: "uploading", uploadedBytes, totalBytes }),
      onSuccess: () => {
        const url = upload.url ?? "";
        const uploadId = url.split("/").pop() ?? "";
        setStatus({ kind: "uploaded", uploadId, filename: file.name });
        qc.invalidateQueries({ queryKey: ["incoming"] });
      },
    });
    uploadRef.current = upload;
    upload.start();
  };

  const onPause = () => {
    const upload = uploadRef.current;
    if (!upload || status.kind !== "uploading") return;
    // abort(false) stops the in-flight PATCH and remembers the URL
    // so the next start() resumes from the last acknowledged byte.
    void upload.abort(false);
    setStatus({
      kind: "paused",
      uploadedBytes: status.uploadedBytes,
      totalBytes: status.totalBytes,
    });
  };

  const onResume = () => {
    const upload = uploadRef.current;
    if (!upload || status.kind !== "paused") return;
    setStatus({
      kind: "uploading",
      uploadedBytes: status.uploadedBytes,
      totalBytes: status.totalBytes,
    });
    upload.start();
  };

  const onIngest = async () => {
    if (status.kind !== "uploaded") return;
    setStatus({ kind: "ingesting", uploadId: status.uploadId });
    try {
      const resp = await api.post<{ ok: boolean; job: string; run_id: string }>(
        "/ingest/bulk",
        { job, upload_id: status.uploadId, source },
      );
      qc.invalidateQueries({ queryKey: ["ingest-jobs"] });
      setStatus({ kind: "done", jobName: resp.job });
      // Hand off to the jobs screen where the curator can watch the
      // run play out.
      setTimeout(() => nav("/jobs"), 1_500);
    } catch (e) {
      setStatus({ kind: "error", message: (e as Error).message });
    }
  };

  return (
    <div className="p-8 max-w-2xl space-y-4">
      <header>
        <h1 className="text-2xl font-semibold">Upload bulk dataset</h1>
        <p className="text-ink-500 text-sm mt-1">
          Resumable upload over the TUS protocol. Drop in a multi-GB
          GeoTIFF (Kartverket DTM10/DTM1), pause and resume across
          network failures, then trigger ingest without ever needing
          shell access to the server.
        </p>
      </header>

      <Field label="Job">
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
      </Field>

      <Field label="Source label (recorded against each loaded row)">
        <input
          className={inputClass}
          value={source}
          onChange={(e) => setSource(e.target.value)}
          placeholder="dtm10"
        />
      </Field>

      {/* Resource is currently unused by dtm-load but threaded so a
          future fkb-bulk job can use it. */}
      <Field label="Target resource (informational)">
        <select
          value={resource}
          onChange={(e) => setResource(e.target.value as Resource)}
          className={inputClass}
        >
          {RESOURCES.map((r) => (
            <option key={r} value={r}>
              {r}
            </option>
          ))}
        </select>
      </Field>

      <Field label="File">
        <input
          type="file"
          accept=".tif,.tiff,.gpkg,.zip"
          onChange={(e) => setFile(e.target.files?.[0] ?? null)}
          className="text-sm"
          data-testid="bulk-file-input"
        />
      </Field>

      {status.kind === "idle" && (
        <button
          type="button"
          onClick={onStart}
          disabled={!file}
          className={primaryBtn}
          data-testid="bulk-start"
        >
          Start upload
        </button>
      )}

      {status.kind === "uploading" && (
        <div className="space-y-2" data-testid="bulk-progress">
          <ProgressBar value={status.uploadedBytes} max={status.totalBytes} />
          <div className="text-xs text-ink-500 tabular-nums">
            {formatBytes(status.uploadedBytes)} /{" "}
            {formatBytes(status.totalBytes)} (
            {((status.uploadedBytes / status.totalBytes) * 100).toFixed(1)}%)
          </div>
          <button
            type="button"
            onClick={onPause}
            className={secondaryBtn}
            data-testid="bulk-pause"
          >
            Pause
          </button>
        </div>
      )}

      {status.kind === "paused" && (
        <div className="space-y-2" data-testid="bulk-paused">
          <ProgressBar value={status.uploadedBytes} max={status.totalBytes} />
          <div className="text-xs text-ink-500 tabular-nums">
            Paused at {formatBytes(status.uploadedBytes)} /{" "}
            {formatBytes(status.totalBytes)} — resume to keep going from
            this offset.
          </div>
          <button
            type="button"
            onClick={onResume}
            className={primaryBtn}
            data-testid="bulk-resume"
          >
            Resume
          </button>
        </div>
      )}

      {status.kind === "uploaded" && (
        <div className="space-y-2">
          <div
            className="text-sm rounded border border-emerald-300 bg-emerald-50 px-3 py-2"
            data-testid="bulk-uploaded"
          >
            Uploaded <strong>{status.filename}</strong> ({status.uploadId.slice(0, 8)}…)
          </div>
          <button
            type="button"
            onClick={onIngest}
            className={primaryBtn}
            data-testid="bulk-ingest"
          >
            Trigger {job} ingest
          </button>
        </div>
      )}

      {status.kind === "ingesting" && (
        <div className="text-sm text-ink-500" data-testid="bulk-ingesting">
          Triggering ingest…
        </div>
      )}

      {status.kind === "done" && (
        <div
          className="text-sm rounded border border-emerald-300 bg-emerald-50 px-3 py-2"
          data-testid="bulk-done"
        >
          {status.jobName} kicked off. Redirecting to Jobs…
        </div>
      )}

      {status.kind === "error" && (
        <div
          className="text-sm rounded border border-red-300 bg-red-50 px-3 py-2"
          data-testid="bulk-error"
        >
          {status.message}
          <button
            type="button"
            onClick={onStart}
            disabled={!file}
            className="ml-3 text-red-700 underline"
          >
            Retry
          </button>
        </div>
      )}
    </div>
  );
}

function Field({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <label className="block">
      <span className="text-sm font-medium text-ink-700">{label}</span>
      <div className="mt-1">{children}</div>
    </label>
  );
}

function ProgressBar({ value, max }: { value: number; max: number }) {
  const pct = max > 0 ? (value / max) * 100 : 0;
  return (
    <div className="h-2 w-full rounded bg-ink-200 overflow-hidden">
      <div
        className="h-full bg-ink-700 transition-all"
        style={{ width: `${pct}%` }}
      />
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
const primaryBtn =
  "px-4 py-2 rounded bg-ink-900 text-ink-50 hover:bg-ink-700 disabled:opacity-50 text-sm";
const secondaryBtn =
  "px-3 py-1.5 rounded border border-ink-200 hover:bg-ink-100 text-sm";
