import { useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { useRouteDetail, useUpdateRoute } from "../api/queries";
import type { Resource, UpdateBody } from "../api/types";
import { MapPreview } from "../components/MapPreview";

export function ResourceEdit() {
  const { resource, id } = useParams<{ resource: Resource; id: string }>();
  const nav = useNavigate();
  const detail = useRouteDetail(resource as Resource, id ?? "");
  const update = useUpdateRoute(resource as Resource, id ?? "");

  const [form, setForm] = useState<UpdateBody>({});
  useEffect(() => {
    if (!detail.data) return;
    setForm({
      name: detail.data.name ?? "",
      description: detail.data.description ?? "",
      difficulty: detail.data.difficulty ?? "",
      marking: detail.data.marking ?? "",
      season: detail.data.season ?? [],
      surface: detail.data.surface ?? "",
      status: detail.data.status,
      attribution: detail.data.attribution ?? "",
    });
  }, [detail.data]);

  if (!resource || !id) return null;
  if (detail.isLoading)
    return <div className="p-8 text-ink-500">Loading…</div>;
  if (!detail.data) return <div className="p-8 text-ink-500">Not found.</div>;

  const onSave = async () => {
    await update.mutateAsync(form);
    nav(`/resources/${resource}`);
  };

  return (
    <div className="p-8 max-w-6xl">
      <header className="mb-6">
        <h1 className="text-2xl font-semibold">
          {detail.data.name ?? detail.data.slug}
        </h1>
        <p className="text-ink-500 text-sm mt-1">
          {resource} · {detail.data.source} ·{" "}
          {detail.data.length_m
            ? `${(detail.data.length_m / 1000).toFixed(2)} km`
            : "no length"}
        </p>
      </header>

      <div className="grid grid-cols-2 gap-6">
        <div className="space-y-4">
          <Field label="Name">
            <input
              type="text"
              value={form.name ?? ""}
              onChange={(e) => setForm({ ...form, name: e.target.value })}
              className={inputClass}
            />
          </Field>
          <Field label="Description">
            <textarea
              value={form.description ?? ""}
              onChange={(e) =>
                setForm({ ...form, description: e.target.value })
              }
              rows={4}
              className={inputClass}
            />
          </Field>
          <div className="grid grid-cols-2 gap-3">
            <Field label="Difficulty">
              <select
                value={form.difficulty ?? ""}
                onChange={(e) =>
                  setForm({ ...form, difficulty: e.target.value })
                }
                className={inputClass}
              >
                <option value="">—</option>
                <option value="easy">Easy</option>
                <option value="medium">Medium</option>
                <option value="hard">Hard</option>
              </select>
            </Field>
            <Field label="Marking">
              <select
                value={form.marking ?? ""}
                onChange={(e) =>
                  setForm({ ...form, marking: e.target.value })
                }
                className={inputClass}
              >
                <option value="">—</option>
                <option value="red">Red</option>
                <option value="blue">Blue</option>
                <option value="yellow">Yellow</option>
                <option value="unmarked">Unmarked</option>
              </select>
            </Field>
          </div>
          <Field label="Surface">
            <input
              type="text"
              value={form.surface ?? ""}
              onChange={(e) => setForm({ ...form, surface: e.target.value })}
              className={inputClass}
            />
          </Field>
          <Field label="Season">
            <div className="flex gap-3 mt-1">
              {(["summer", "winter"] as const).map((s) => (
                <label key={s} className="flex items-center gap-2 text-sm">
                  <input
                    type="checkbox"
                    checked={form.season?.includes(s) ?? false}
                    onChange={(e) => {
                      const current = form.season ?? [];
                      setForm({
                        ...form,
                        season: e.target.checked
                          ? [...current, s]
                          : current.filter((x) => x !== s),
                      });
                    }}
                  />
                  {s}
                </label>
              ))}
            </div>
          </Field>
          <Field label="Status">
            <select
              value={form.status ?? "draft"}
              onChange={(e) =>
                setForm({
                  ...form,
                  status: e.target.value as UpdateBody["status"],
                })
              }
              className={inputClass}
            >
              <option value="draft">Draft</option>
              <option value="published">Published</option>
              <option value="archived">Archived</option>
            </select>
          </Field>
          <Field label="Attribution">
            <input
              type="text"
              value={form.attribution ?? ""}
              onChange={(e) =>
                setForm({ ...form, attribution: e.target.value })
              }
              className={inputClass}
            />
          </Field>
          <div className="flex gap-2 pt-2">
            <button
              type="button"
              onClick={onSave}
              disabled={update.isPending}
              className="px-4 py-2 rounded bg-ink-900 text-ink-50 hover:bg-ink-700 disabled:opacity-50"
            >
              {update.isPending ? "Saving…" : "Save"}
            </button>
            <button
              type="button"
              onClick={() => nav(`/resources/${resource}`)}
              className="px-4 py-2 rounded border border-ink-200 hover:bg-ink-100"
            >
              Cancel
            </button>
          </div>
          {update.error && (
            <div className="text-sm text-red-700">
              {(update.error as Error).message}
            </div>
          )}
        </div>

        <div>
          <div className="text-sm font-medium text-ink-700 mb-2">
            Geometry preview
          </div>
          <MapPreview geometry={detail.data.geometry} resource={resource} />
        </div>
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
