import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { useUploadGpx } from "../api/queries";
import { RESOURCES, type Resource } from "../api/types";

export function UploadGpx() {
  const nav = useNavigate();
  const [file, setFile] = useState<File | null>(null);
  const [resource, setResource] = useState<Resource>("hiking-trails");
  const [name, setName] = useState("");
  const upload = useUploadGpx();

  const onSubmit = async () => {
    if (!file) return;
    const form = new FormData();
    form.append("file", file);
    form.append("resource", resource);
    if (name) form.append("name", name);
    const res = await upload.mutateAsync(form);
    nav(`/resources/${res.resource}/${res.id}`);
  };

  return (
    <div className="p-8 max-w-2xl space-y-4">
      <header>
        <h1 className="text-2xl font-semibold">Upload GPX</h1>
        <p className="text-ink-500 text-sm mt-1">
          Imports the GPX track segments as a draft curated route. You can
          set difficulty, marking, and publish in the next step.
        </p>
      </header>
      <Field label="Resource">
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
      <Field label="Name (optional — falls back to GPX track name)">
        <input
          className={inputClass}
          value={name}
          onChange={(e) => setName(e.target.value)}
        />
      </Field>
      <Field label="GPX file (max 25 MB)">
        <input
          type="file"
          accept=".gpx,application/gpx+xml,application/octet-stream"
          onChange={(e) => setFile(e.target.files?.[0] ?? null)}
          className="text-sm"
        />
      </Field>
      <button
        type="button"
        onClick={onSubmit}
        disabled={!file || upload.isPending}
        className="px-4 py-2 rounded bg-ink-900 text-ink-50 hover:bg-ink-700 disabled:opacity-50"
      >
        {upload.isPending ? "Uploading…" : "Upload"}
      </button>
      {upload.error && (
        <div className="text-sm text-red-700">
          {(upload.error as Error).message}
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

const inputClass =
  "w-full px-3 py-2 text-sm border border-ink-200 rounded bg-white focus:outline-none focus:ring-2 focus:ring-ink-700";
