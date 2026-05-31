/**
 * Primitives — the agentic test/debug surface for every geo primitive.
 *
 * Stages 1 (elev) + 2 (slope) are functional. Later stages append
 * tabs without disturbing the others.
 */

import { useQuery } from "@tanstack/react-query";
import { useState } from "react";

import {
  v1,
  type DemCoverage,
  type ElevBenchResp,
  type ElevSampleResp,
  type GraphStats,
  type LayersResp,
  type MaskCoverage,
  type MaskSampleResp,
  type NameResp,
  type NearestResp,
  type PathfindResp,
  type Profile,
  type RouteResp,
  type SearchCoverage,
  type SlopeSampleResp,
} from "../api/v1";
import { ApiError } from "../api/client";

const TABS = [
  { id: "elevation", label: "Elevation", stage: 1 },
  { id: "slope", label: "Slope / Aspect", stage: 2 },
  { id: "mask", label: "Refusal mask", stage: 3 },
  { id: "graph", label: "Routing graph", stage: 4 },
  { id: "search", label: "Search", stage: 5 },
  { id: "pathfind", label: "Off-trail pathfind", stage: 6 },
];

export function Primitives() {
  const [active, setActive] = useState<string>("elevation");

  return (
    <div className="p-8 max-w-6xl space-y-6">
      <header>
        <h1 className="text-2xl font-semibold">Primitives</h1>
        <p className="text-ink-500 text-sm mt-1">
          Inspect and benchmark every geo primitive backed by mmap'd
          artifacts. Each tab drives <code>/v1/&lt;p&gt;/*</code> and{" "}
          <code>/v1/debug/&lt;p&gt;/*</code>.
        </p>
      </header>

      <nav className="flex gap-2 border-b border-ink-200">
        {TABS.map((t) => (
          <button
            key={t.id}
            type="button"
            onClick={() => setActive(t.id)}
            className={`px-3 py-2 text-sm border-b-2 -mb-px ${
              active === t.id
                ? "border-ink-900 font-medium"
                : "border-transparent text-ink-500 hover:text-ink-700"
            }`}
          >
            {t.label}
            <span className="ml-1 text-xs text-ink-400">S{t.stage}</span>
          </button>
        ))}
      </nav>

      <section className="rounded border border-ink-200 bg-white p-6">
        {active === "elevation" ? (
          <ElevationPanel />
        ) : active === "slope" ? (
          <SlopePanel />
        ) : active === "mask" ? (
          <MaskPanel />
        ) : active === "graph" ? (
          <GraphPanel />
        ) : active === "search" ? (
          <SearchPanel />
        ) : active === "pathfind" ? (
          <PathfindPanel />
        ) : (
          <PlaceholderPanel tabId={active} />
        )}
      </section>
    </div>
  );
}

function ElevationPanel() {
  const coverage = useQuery({
    queryKey: ["elev", "coverage"],
    queryFn: () => v1.get<DemCoverage>("/debug/elev/coverage"),
    retry: false,
  });
  const [lon, setLon] = useState("10.7522");
  const [lat, setLat] = useState("59.9139");
  const [sample, setSample] = useState<ElevSampleResp | null>(null);
  const [sampleErr, setSampleErr] = useState<string | null>(null);
  const [bench, setBench] = useState<ElevBenchResp | null>(null);
  const [benching, setBenching] = useState(false);

  const runSample = async () => {
    setSampleErr(null);
    try {
      const r = await v1.post<ElevSampleResp>("/elev/sample", {
        lon: Number(lon),
        lat: Number(lat),
      });
      setSample(r);
    } catch (e) {
      setSample(null);
      setSampleErr(formatApiError(e));
    }
  };

  const runBench = async () => {
    setBench(null);
    setBenching(true);
    try {
      const r = await v1.get<ElevBenchResp>("/debug/elev/bench");
      setBench(r);
    } catch (e) {
      setSampleErr(formatApiError(e));
    } finally {
      setBenching(false);
    }
  };

  return (
    <div className="space-y-6">
      <h2 className="text-lg font-medium">Elevation primitive (Stage 1)</h2>

      <div>
        <h3 className="text-sm font-medium text-ink-700 mb-2">Coverage</h3>
        {coverage.isLoading ? (
          <div className="text-sm text-ink-500">Loading…</div>
        ) : coverage.isError ? (
          <Degraded err={coverage.error} />
        ) : coverage.data ? (
          <CoverageTable cov={coverage.data} />
        ) : null}
      </div>

      <div>
        <h3 className="text-sm font-medium text-ink-700 mb-2">Sample point</h3>
        <div className="flex items-end gap-2 flex-wrap">
          <Field label="lon" value={lon} onChange={setLon} width="w-32" />
          <Field label="lat" value={lat} onChange={setLat} width="w-32" />
          <button
            type="button"
            onClick={runSample}
            className="px-3 py-1.5 rounded bg-ink-900 text-ink-50 text-sm hover:bg-ink-800"
          >
            POST /v1/elev/sample
          </button>
        </div>
        {sampleErr ? (
          <div className="text-rose-700 text-sm mt-2">{sampleErr}</div>
        ) : null}
        {sample ? (
          <pre className="mt-3 text-xs bg-ink-50 border border-ink-200 rounded p-3 overflow-x-auto">
            {JSON.stringify(sample, null, 2)}
          </pre>
        ) : null}
      </div>

      <div>
        <h3 className="text-sm font-medium text-ink-700 mb-2">Benchmark</h3>
        <button
          type="button"
          onClick={runBench}
          disabled={benching}
          className="px-3 py-1.5 rounded bg-ink-900 text-ink-50 text-sm hover:bg-ink-800 disabled:opacity-50"
        >
          {benching ? "Running…" : "Run /debug/elev/bench"}
        </button>
        {bench ? (
          <pre className="mt-3 text-xs bg-ink-50 border border-ink-200 rounded p-3 overflow-x-auto">
            {JSON.stringify(bench, null, 2)}
          </pre>
        ) : null}
      </div>
    </div>
  );
}

function SlopePanel() {
  const [lon, setLon] = useState("10.7522");
  const [lat, setLat] = useState("59.9139");
  const [result, setResult] = useState<SlopeSampleResp | null>(null);
  const [err, setErr] = useState<string | null>(null);

  const run = async () => {
    setErr(null);
    try {
      const r = await v1.post<SlopeSampleResp>("/slope/sample", {
        lon: Number(lon),
        lat: Number(lat),
      });
      setResult(r);
    } catch (e) {
      setResult(null);
      setErr(formatApiError(e));
    }
  };

  return (
    <div className="space-y-4">
      <h2 className="text-lg font-medium">Slope / Aspect (Stage 2)</h2>
      <p className="text-sm text-ink-500">
        Horn (1981) central differences over the 3×3 neighbourhood in
        the DEM. Shares the elevation artifact — no separate boot.
      </p>
      <div className="flex items-end gap-2 flex-wrap">
        <Field label="lon" value={lon} onChange={setLon} width="w-32" />
        <Field label="lat" value={lat} onChange={setLat} width="w-32" />
        <button
          type="button"
          onClick={run}
          className="px-3 py-1.5 rounded bg-ink-900 text-ink-50 text-sm hover:bg-ink-800"
        >
          POST /v1/slope/sample
        </button>
      </div>
      {err ? <div className="text-rose-700 text-sm">{err}</div> : null}
      {result ? (
        <pre className="text-xs bg-ink-50 border border-ink-200 rounded p-3 overflow-x-auto">
          {JSON.stringify(result, null, 2)}
        </pre>
      ) : null}
    </div>
  );
}

function MaskPanel() {
  const coverage = useQuery({
    queryKey: ["mask", "coverage"],
    queryFn: () => v1.get<MaskCoverage>("/debug/mask/coverage"),
    retry: false,
  });
  const [lon, setLon] = useState("10.7522");
  const [lat, setLat] = useState("59.9139");
  const [sample, setSample] = useState<MaskSampleResp | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const run = async () => {
    setErr(null);
    try {
      const r = await v1.post<MaskSampleResp>("/mask/sample", {
        lon: Number(lon),
        lat: Number(lat),
      });
      setSample(r);
    } catch (e) {
      setSample(null);
      setErr(formatApiError(e));
    }
  };
  return (
    <div className="space-y-6">
      <h2 className="text-lg font-medium">Refusal mask (Stage 3)</h2>
      <p className="text-sm text-ink-500">
        2-bit packed bitmap at 100 m. Reader + format are live; the
        rasterising builder is deferred (run a synthetic mask via
        Rust integration tests today).
      </p>
      <div>
        <h3 className="text-sm font-medium text-ink-700 mb-2">Coverage</h3>
        {coverage.isLoading ? (
          <div className="text-sm text-ink-500">Loading…</div>
        ) : coverage.isError ? (
          <Degraded err={coverage.error} />
        ) : coverage.data ? (
          <pre className="text-xs bg-ink-50 border border-ink-200 rounded p-3 overflow-x-auto">
            {JSON.stringify(coverage.data, null, 2)}
          </pre>
        ) : null}
      </div>
      <div className="flex items-end gap-2 flex-wrap">
        <Field label="lon" value={lon} onChange={setLon} width="w-32" />
        <Field label="lat" value={lat} onChange={setLat} width="w-32" />
        <button
          type="button"
          onClick={run}
          className="px-3 py-1.5 rounded bg-ink-900 text-ink-50 text-sm hover:bg-ink-800"
        >
          POST /v1/mask/sample
        </button>
      </div>
      {err ? <div className="text-rose-700 text-sm">{err}</div> : null}
      {sample ? (
        <pre className="text-xs bg-ink-50 border border-ink-200 rounded p-3 overflow-x-auto">
          {JSON.stringify(sample, null, 2)}
        </pre>
      ) : null}
    </div>
  );
}

function GraphPanel() {
  const stats = useQuery({
    queryKey: ["graph", "stats"],
    queryFn: () => v1.get<GraphStats>("/debug/graph/stats"),
    retry: false,
  });
  const [fromLon, setFromLon] = useState("10.7522");
  const [fromLat, setFromLat] = useState("59.9139");
  const [toLon, setToLon] = useState("10.7600");
  const [toLat, setToLat] = useState("59.9200");
  const [profile, setProfile] = useState<Profile>("foot");
  const [result, setResult] = useState<RouteResp | null>(null);
  const [err, setErr] = useState<string | null>(null);

  const run = async () => {
    setErr(null);
    try {
      const r = await v1.post<RouteResp>("/route", {
        from: [Number(fromLon), Number(fromLat)],
        to: [Number(toLon), Number(toLat)],
        profile,
        snap_radius_m: 2000,
      });
      setResult(r);
    } catch (e) {
      setResult(null);
      setErr(formatApiError(e));
    }
  };

  return (
    <div className="space-y-6">
      <h2 className="text-lg font-medium">Routing graph (Stage 4)</h2>
      <p className="text-sm text-ink-500">
        CSR adjacency mmap'd from <code>norway.graph</code>;
        Dijkstra with per-profile baked costs. Profiles:{" "}
        <code>foot</code>, <code>bicycle</code>, <code>ski</code>.
      </p>
      <div>
        <h3 className="text-sm font-medium text-ink-700 mb-2">Stats</h3>
        {stats.isLoading ? (
          <div className="text-sm text-ink-500">Loading…</div>
        ) : stats.isError ? (
          <Degraded err={stats.error} />
        ) : stats.data ? (
          <pre className="text-xs bg-ink-50 border border-ink-200 rounded p-3 overflow-x-auto">
            {JSON.stringify(stats.data, null, 2)}
          </pre>
        ) : null}
      </div>
      <div className="grid grid-cols-2 gap-3">
        <Field label="from lon" value={fromLon} onChange={setFromLon} />
        <Field label="from lat" value={fromLat} onChange={setFromLat} />
        <Field label="to lon" value={toLon} onChange={setToLon} />
        <Field label="to lat" value={toLat} onChange={setToLat} />
      </div>
      <div className="flex items-end gap-2 flex-wrap">
        <label className="text-sm flex flex-col gap-1">
          <span className="text-ink-500">profile</span>
          <select
            value={profile}
            onChange={(e) => setProfile(e.target.value as Profile)}
            className="px-2 py-1.5 rounded border border-ink-200 text-xs"
          >
            <option value="foot">foot</option>
            <option value="bicycle">bicycle</option>
            <option value="ski">ski</option>
          </select>
        </label>
        <button
          type="button"
          onClick={run}
          className="px-3 py-1.5 rounded bg-ink-900 text-ink-50 text-sm hover:bg-ink-800"
        >
          POST /v1/route
        </button>
      </div>
      {err ? <div className="text-rose-700 text-sm">{err}</div> : null}
      {result ? (
        <pre className="text-xs bg-ink-50 border border-ink-200 rounded p-3 overflow-x-auto max-h-96">
          {JSON.stringify(result, null, 2)}
        </pre>
      ) : null}
    </div>
  );
}

function SearchPanel() {
  const coverage = useQuery({
    queryKey: ["search", "coverage"],
    queryFn: () => v1.get<SearchCoverage>("/debug/search/coverage"),
    retry: false,
  });
  const [lon, setLon] = useState("10.7522");
  const [lat, setLat] = useState("59.9139");
  const [name, setName] = useState("Galdh");
  const [near, setNear] = useState<NearestResp | null>(null);
  const [hits, setHits] = useState<NameResp | null>(null);
  const [err, setErr] = useState<string | null>(null);

  const runNearest = async () => {
    setErr(null);
    try {
      const r = await v1.post<NearestResp>("/search/nearest", {
        lon: Number(lon),
        lat: Number(lat),
        n: 10,
      });
      setNear(r);
    } catch (e) {
      setNear(null);
      setErr(formatApiError(e));
    }
  };
  const runName = async () => {
    setErr(null);
    try {
      const r = await v1.get<NameResp>(
        `/search/name?q=${encodeURIComponent(name)}&limit=10`,
      );
      setHits(r);
    } catch (e) {
      setHits(null);
      setErr(formatApiError(e));
    }
  };

  return (
    <div className="space-y-6">
      <h2 className="text-lg font-medium">Anchor search (Stage 5)</h2>
      <div>
        <h3 className="text-sm font-medium text-ink-700 mb-2">Coverage</h3>
        {coverage.isLoading ? (
          <div className="text-sm text-ink-500">Loading…</div>
        ) : coverage.isError ? (
          <Degraded err={coverage.error} />
        ) : coverage.data ? (
          <pre className="text-xs bg-ink-50 border border-ink-200 rounded p-3 overflow-x-auto">
            {JSON.stringify(coverage.data, null, 2)}
          </pre>
        ) : null}
      </div>
      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-2">
          <h3 className="text-sm font-medium text-ink-700">Nearest</h3>
          <Field label="lon" value={lon} onChange={setLon} />
          <Field label="lat" value={lat} onChange={setLat} />
          <button
            type="button"
            onClick={runNearest}
            className="px-3 py-1.5 rounded bg-ink-900 text-ink-50 text-sm hover:bg-ink-800"
          >
            POST /v1/search/nearest
          </button>
        </div>
        <div className="space-y-2">
          <h3 className="text-sm font-medium text-ink-700">Name</h3>
          <Field label="q" value={name} onChange={setName} />
          <button
            type="button"
            onClick={runName}
            className="px-3 py-1.5 rounded bg-ink-900 text-ink-50 text-sm hover:bg-ink-800"
          >
            GET /v1/search/name
          </button>
        </div>
      </div>
      {err ? <div className="text-rose-700 text-sm">{err}</div> : null}
      {near ? (
        <div>
          <div className="text-xs text-ink-500 mb-1">Nearest (took {near.took_us} µs)</div>
          <pre className="text-xs bg-ink-50 border border-ink-200 rounded p-3 overflow-x-auto max-h-64">
            {JSON.stringify(near.anchors, null, 2)}
          </pre>
        </div>
      ) : null}
      {hits ? (
        <div>
          <div className="text-xs text-ink-500 mb-1">Name match (took {hits.took_us} µs)</div>
          <pre className="text-xs bg-ink-50 border border-ink-200 rounded p-3 overflow-x-auto max-h-64">
            {JSON.stringify(hits.anchors, null, 2)}
          </pre>
        </div>
      ) : null}
    </div>
  );
}

function PathfindPanel() {
  const [fromLon, setFromLon] = useState("10.7522");
  const [fromLat, setFromLat] = useState("59.9139");
  const [toLon, setToLon] = useState("10.7600");
  const [toLat, setToLat] = useState("59.9200");
  const [allowOffTrail, setAllowOffTrail] = useState(true);
  const [profile, setProfile] = useState<Profile>("foot");
  const [snapRadius, setSnapRadius] = useState("200");
  const [bridgeRadius, setBridgeRadius] = useState("3000");
  const [meshCell, setMeshCell] = useState("100");
  const [result, setResult] = useState<PathfindResp | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [weights, setWeights] = useState<Record<string, number>>({});

  const layers = useQuery({
    queryKey: ["pathfind", "layers"],
    queryFn: () => v1.get<LayersResp>("/debug/pathfind/layers"),
    retry: false,
  });

  const setLayerWeight = (name: string, w: number) =>
    setWeights((prev) => ({ ...prev, [name]: w }));

  const run = async () => {
    setErr(null);
    try {
      const r = await v1.post<PathfindResp>("/pathfind", {
        from: [Number(fromLon), Number(fromLat)],
        to: [Number(toLon), Number(toLat)],
        prefs: {
          profile,
          allow_off_trail: allowOffTrail,
          snap_radius_m: Number(snapRadius),
          bridge_radius_m: Number(bridgeRadius),
          mesh_cell_m: Number(meshCell),
          max_off_trail_km: 10,
          layer_weights: weights,
        },
      });
      setResult(r);
    } catch (e) {
      setResult(null);
      setErr(formatApiError(e));
    }
  };

  return (
    <div className="space-y-6">
      <h2 className="text-lg font-medium">Off-trail pathfind (Stage 6)</h2>
      <p className="text-sm text-ink-500">
        Tries on-graph → hybrid (off-trail prefix → graph middle →
        off-trail suffix) → pure off-trail. Cost is composed from
        the layer stack shown below; per-request weights let you
        suppress or boost individual layers without rebooting.
      </p>

      <div>
        <h3 className="text-sm font-medium text-ink-700 mb-2">Layer weights</h3>
        {layers.isLoading ? (
          <div className="text-sm text-ink-500">Loading layers…</div>
        ) : layers.isError ? (
          <Degraded err={layers.error} />
        ) : layers.data ? (
          layers.data.layers.length === 0 ? (
            <div className="text-sm text-ink-500">
              No layers registered. Load DEM/mask artifacts or add custom
              layers at boot.
            </div>
          ) : (
            <div className="grid grid-cols-2 gap-2">
              {layers.data.layers.map((name) => (
                <label key={name} className="text-sm flex items-center gap-3">
                  <code className="text-xs bg-ink-100 px-1 py-0.5 rounded w-32">
                    {name}
                  </code>
                  <input
                    type="range"
                    min={0}
                    max={2}
                    step={0.1}
                    value={weights[name] ?? 1.0}
                    onChange={(e) => setLayerWeight(name, Number(e.target.value))}
                    className="flex-1"
                  />
                  <span className="text-xs text-ink-500 w-10 text-right">
                    {(weights[name] ?? 1.0).toFixed(1)}×
                  </span>
                </label>
              ))}
            </div>
          )
        ) : null}
      </div>

      <div>
        <h3 className="text-sm font-medium text-ink-700 mb-2">Endpoints</h3>
        <div className="grid grid-cols-2 gap-3">
          <Field label="from lon" value={fromLon} onChange={setFromLon} />
          <Field label="from lat" value={fromLat} onChange={setFromLat} />
          <Field label="to lon" value={toLon} onChange={setToLon} />
          <Field label="to lat" value={toLat} onChange={setToLat} />
        </div>
      </div>

      <div>
        <h3 className="text-sm font-medium text-ink-700 mb-2">Strategy controls</h3>
        <div className="grid grid-cols-3 gap-3">
          <Field label="snap_radius_m" value={snapRadius} onChange={setSnapRadius} />
          <Field
            label="bridge_radius_m"
            value={bridgeRadius}
            onChange={setBridgeRadius}
          />
          <Field label="mesh_cell_m" value={meshCell} onChange={setMeshCell} />
        </div>
      </div>

      <div className="flex items-end gap-3 flex-wrap">
        <label className="text-sm flex flex-col gap-1">
          <span className="text-ink-500">profile</span>
          <select
            value={profile}
            onChange={(e) => setProfile(e.target.value as Profile)}
            className="px-2 py-1.5 rounded border border-ink-200 text-xs"
          >
            <option value="foot">foot</option>
            <option value="bicycle">bicycle</option>
            <option value="ski">ski</option>
          </select>
        </label>
        <label className="text-sm flex items-center gap-2">
          <input
            type="checkbox"
            checked={allowOffTrail}
            onChange={(e) => setAllowOffTrail(e.target.checked)}
          />
          <span>allow off-trail</span>
        </label>
        <button
          type="button"
          onClick={run}
          className="px-3 py-1.5 rounded bg-ink-900 text-ink-50 text-sm hover:bg-ink-800"
        >
          POST /v1/pathfind
        </button>
      </div>
      {err ? <div className="text-rose-700 text-sm">{err}</div> : null}
      {result ? (
        <div className="space-y-3">
          <div className="text-xs text-ink-500">
            strategy=<b>{result.path.strategy}</b> length=
            {result.path.length_m.toFixed(0)} m cost=
            {result.path.cost.toFixed(1)} on_trail_pct=
            {result.path.on_trail_pct.toFixed(1)} (took {result.took_us} µs)
          </div>
          {result.path.refused_by.length > 0 ? (
            <div className="text-xs text-amber-700">
              Cells refused by: {result.path.refused_by.join(", ")}
            </div>
          ) : null}
          {result.path.legs.length > 0 ? (
            <table className="text-xs w-full">
              <thead>
                <tr className="text-ink-500 text-left">
                  <th className="py-1">leg</th>
                  <th className="py-1">kind</th>
                  <th className="py-1">length m</th>
                  <th className="py-1">verts</th>
                </tr>
              </thead>
              <tbody>
                {result.path.legs.map((l, i) => (
                  <tr key={i}>
                    <td className="py-1">{i}</td>
                    <td className="py-1 font-mono">{l.kind}</td>
                    <td className="py-1">{l.length_m.toFixed(0)}</td>
                    <td className="py-1">
                      {l.end_idx - l.start_idx + 1}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : null}
          <pre className="text-xs bg-ink-50 border border-ink-200 rounded p-3 overflow-x-auto max-h-72">
            {JSON.stringify(result.path, null, 2)}
          </pre>
        </div>
      ) : null}
    </div>
  );
}

function PlaceholderPanel({ tabId }: { tabId: string }) {
  const tab = TABS.find((t) => t.id === tabId);
  if (!tab) return null;
  return (
    <div className="text-sm text-ink-600 space-y-2">
      <div className="font-medium text-ink-900">{tab.label} primitive</div>
      <p>
        Lands in Stage {tab.stage}. Until then,{" "}
        <code className="text-xs bg-ink-100 px-1 py-0.5 rounded">
          /v1/{tabId}/*
        </code>{" "}
        returns 503.
      </p>
    </div>
  );
}

function CoverageTable({ cov }: { cov: DemCoverage }) {
  const rows: [string, string | number][] = [
    ["cells", `${cov.cells_x} × ${cov.cells_y}`],
    ["resolution_m", cov.resolution_m],
    ["tiles", `${cov.tiles_x} × ${cov.tiles_y}  (${cov.tiles_present} present, ${cov.tiles_absent} absent)`],
    ["extent (EPSG:25833)", `${cov.min_x.toFixed(0)},${cov.min_y.toFixed(0)} → ${cov.max_x.toFixed(0)},${cov.max_y.toFixed(0)}`],
    ["file_size_bytes", cov.file_size_bytes.toLocaleString()],
    ["built", new Date(cov.build_timestamp_unix_sec * 1000).toISOString()],
    [
      "cache",
      `${cov.cache.entries} tiles, ${(cov.cache.total_bytes / 1024 / 1024).toFixed(1)} / ${(cov.cache.capacity_bytes / 1024 / 1024).toFixed(0)} MiB, ${cov.cache.hits} hits / ${cov.cache.misses} misses / ${cov.cache.evictions} evictions`,
    ],
  ];
  return (
    <table className="text-sm w-full">
      <tbody>
        {rows.map(([k, v]) => (
          <tr key={k}>
            <td className="text-ink-500 pr-4 py-1 align-top w-48">{k}</td>
            <td className="font-mono text-xs py-1">{v}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function Field({
  label,
  value,
  onChange,
  width = "w-40",
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  width?: string;
}) {
  return (
    <label className="text-sm flex flex-col gap-1">
      <span className="text-ink-500">{label}</span>
      <input
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className={`${width} px-2 py-1.5 rounded border border-ink-200 font-mono text-xs`}
      />
    </label>
  );
}

function Degraded({ err }: { err: unknown }) {
  return (
    <div className="text-sm text-ink-600 bg-amber-50 border border-amber-200 rounded p-3">
      <div className="font-medium text-amber-900">Primitive unavailable</div>
      <div className="text-xs mt-1">{formatApiError(err)}</div>
      <div className="text-xs mt-2 text-ink-500">
        Run <code>tileserver build-artifacts --kind=dem --out=…</code>{" "}
        and boot with <code>--artifacts-dir=…</code>.
      </div>
    </div>
  );
}

function formatApiError(e: unknown): string {
  if (e instanceof ApiError) {
    const body = e.body as { error?: string } | undefined;
    return `HTTP ${e.status}: ${body?.error ?? JSON.stringify(e.body)}`;
  }
  return String(e);
}
