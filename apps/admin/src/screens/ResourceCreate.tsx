import { useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { useCreateRoute } from "../api/queries";
import type { CreateBody, Resource } from "../api/types";

/**
 * Minimal create flow: the curator pastes a GeoJSON LineString or
 * MultiLineString plus a slug+name. For drawing-on-map UX (M4
 * stretch), wire MapLibre Draw into MapPreview later.
 */
export function ResourceCreate() {
  const { resource } = useParams<{ resource: Resource }>();
  const nav = useNavigate();
  const create = useCreateRoute(resource as Resource);
  const [body, setBody] = useState<CreateBody>({
    slug: "",
    name: "",
    geometry: { type: "MultiLineString", coordinates: [] },
  });
  const [geomText, setGeomText] = useState("");
  const [geomError, setGeomError] = useState<string | null>(null);

  if (!resource) return null;

  const onSubmit = async () => {
    try {
      const geom = JSON.parse(geomText);
      setGeomError(null);
      const res = await create.mutateAsync({ ...body, geometry: geom });
      nav(`/resources/${resource}/${res.id}`);
    } catch (e) {
      setGeomError((e as Error).message);
    }
  };

  return (
    <div className="p-8 max-w-3xl space-y-4">
      <header className="mb-2">
        <h1 className="text-2xl font-semibold">Create route</h1>
        <p className="text-ink-500 text-sm mt-1">{resource}</p>
      </header>
      <Field label="Slug (URL identifier)">
        <input
          className={inputClass}
          value={body.slug}
          onChange={(e) => setBody({ ...body, slug: e.target.value })}
          placeholder="e.g. besseggen-ridge"
        />
      </Field>
      <Field label="Name">
        <input
          className={inputClass}
          value={body.name ?? ""}
          onChange={(e) => setBody({ ...body, name: e.target.value })}
        />
      </Field>
      <Field label="Description">
        <textarea
          className={inputClass}
          rows={3}
          value={body.description ?? ""}
          onChange={(e) => setBody({ ...body, description: e.target.value })}
        />
      </Field>
      <Field label="Geometry (GeoJSON LineString or MultiLineString, WGS84)">
        <textarea
          className={inputClass + " font-mono text-xs"}
          rows={8}
          value={geomText}
          onChange={(e) => setGeomText(e.target.value)}
          placeholder='{"type":"LineString","coordinates":[[10.7,59.9],[10.8,60.0]]}'
        />
        {geomError && (
          <div className="text-sm text-red-700 mt-1">{geomError}</div>
        )}
      </Field>
      <div className="flex gap-2 pt-2">
        <button
          type="button"
          onClick={onSubmit}
          disabled={create.isPending}
          className="px-4 py-2 rounded bg-ink-900 text-ink-50 hover:bg-ink-700 disabled:opacity-50"
        >
          {create.isPending ? "Creating…" : "Create draft"}
        </button>
        <button
          type="button"
          onClick={() => nav(`/resources/${resource}`)}
          className="px-4 py-2 rounded border border-ink-200 hover:bg-ink-100"
        >
          Cancel
        </button>
      </div>
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
