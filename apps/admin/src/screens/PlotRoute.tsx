/**
 * The thing the user actually wants:
 *
 * - A map.
 * - Click twice → start + end markers.
 * - The service computes a path.
 * - The result is drawn back on the map, colour-coded by leg.
 * - Side panel shows the active layer stack, lets you scale individual
 *   layers, lists the strategy + length + per-leg breakdown.
 *
 * Layers are queried at mount so the panel reflects whatever the
 * tileserver was booted with (slope, mask_refusal, preferred_edge,
 * marking — plus any custom `CostLayer`s registered at boot).
 */

import { useEffect, useMemo, useRef, useState } from "react";
import maplibregl from "maplibre-gl";
import "maplibre-gl/dist/maplibre-gl.css";

import {
  v1,
  type AnchorsBboxResp,
  type CellInspectResp,
  type DemCoverage,
  type EdgesResp,
  type GraphDensity,
  type GraphStats,
  type InspectResp,
  type LayersResp,
  type MaskCellsResp,
  type MaskCoverage,
  type PathfindResp,
  type Profile,
} from "../api/v1";

// Geographic centre of the seeded synthetic data so the map opens on
// it. For a real Norway-wide deployment, replace with the DEM
// coverage centroid (or fly there once `DemCoverage` lands).
const FALLBACK_CENTER: [number, number] = [10.75, 59.935];
const FALLBACK_ZOOM = 13;

// Okabe–Ito colour-blind-safe palette. Blue vs vermillion is the
// canonical CVD-safe pair (distinct in hue AND lightness for protan/
// deutan/tritan).
const CVD_BLUE = "#0072B2"; // on-trail / graph leg + start marker + live preview
const CVD_VERMILLION = "#D55E00"; // off-trail leg + end marker
const LEG_COLOR = {
  off_trail_prefix: CVD_VERMILLION,
  graph: CVD_BLUE,
  off_trail_suffix: CVD_VERMILLION,
};

type Marker = [number, number]; // [lon, lat]

/**
 * Kartverket's public WMTS cache. RESTful URL pattern is
 * `{base}/{layer}/{style}/{tileMatrixSet}/{z}/{y}/{x}.{format}`.
 * MapLibre substitutes the placeholders by name so the WMTS-style
 * z/y/x ordering Just Works.
 *
 * Layers picked: `topo` is the Topografisk norgeskart (the standard
 * Norwegian hiking-grade map with 5/10/20 m contours, marked trails,
 * cabin/summit names). Greyscale + orthophoto are alternative bases
 * curators can switch to when they want a quieter background or
 * satellite imagery.
 *
 * Kartverket's data licence (NLOD/CC-BY 4.0) requires attribution.
 */
const BASEMAPS = {
  topo: {
    label: "Norgeskart (topo)",
    tiles: [
      "https://cache.kartverket.no/v1/wmts/1.0.0/topo/default/webmercator/{z}/{y}/{x}.png",
    ],
    attribution: "© Kartverket",
  },
  topograatone: {
    label: "Norgeskart (grå)",
    tiles: [
      "https://cache.kartverket.no/v1/wmts/1.0.0/topograatone/default/webmercator/{z}/{y}/{x}.png",
    ],
    attribution: "© Kartverket",
  },
  flyfoto: {
    label: "Flybilder (NIB)",
    tiles: [
      "https://cache.kartverket.no/v1/wmts/1.0.0/Nibcache_web_mercator_v2/default/default028mm/{z}/{y}/{x}.jpeg",
    ],
    attribution: "© Norge i bilder",
  },
  osm: {
    label: "OpenStreetMap",
    tiles: ["https://tile.openstreetmap.org/{z}/{x}/{y}.png"],
    attribution: "© OpenStreetMap contributors",
  },
} as const;

type BasemapId = keyof typeof BASEMAPS;

export function PlotRoute() {
  const mapRef = useRef<maplibregl.Map | null>(null);
  const containerRef = useRef<HTMLDivElement | null>(null);
  // One MapLibre marker per waypoint, rebuilt whenever `points` change.
  const waypointMarkersRef = useRef<maplibregl.Marker[]>([]);

  // Ordered waypoint list — the single source of truth for routing.
  // `from`/`to` below are DERIVED from it (first/last) so the existing
  // endpoint-keyed overlays (inspect, mesh, coverage) keep working.
  const [points, setPoints] = useState<Marker[]>([]);
  const pointsRef = useRef<Marker[]>([]);
  pointsRef.current = points;
  // Undo/redo stacks for waypoint edits (add/move/delete/reorder).
  const [pointsPast, setPointsPast] = useState<Marker[][]>([]);
  const [pointsFuture, setPointsFuture] = useState<Marker[][]>([]);
  // Commit a new waypoint list, pushing the CURRENT list onto the undo
  // stack and clearing the redo stack. Reads current via the ref so it
  // is safe to call from once-bound map/marker event closures.
  const commitPoints = (next: Marker[]) => {
    setPointsPast((p) => [...p, pointsRef.current]);
    setPointsFuture([]);
    setPoints(next);
  };
  const undoPoints = () => {
    setPointsPast((past) => {
      if (past.length === 0) return past;
      const prev = past[past.length - 1];
      setPointsFuture((f) => [pointsRef.current, ...f]);
      setPoints(prev);
      return past.slice(0, -1);
    });
  };
  const redoPoints = () => {
    setPointsFuture((future) => {
      if (future.length === 0) return future;
      const next = future[0];
      setPointsPast((p) => [...p, pointsRef.current]);
      setPoints(next);
      return future.slice(1);
    });
  };

  const [from, setFrom] = useState<Marker | null>(null);
  const [to, setTo] = useState<Marker | null>(null);
  const [profile, setProfile] = useState<Profile>("foot");
  // Trip-style preset (server-resolved cost bundle). Default "balanced".
  const [preset, setPreset] = useState<string>(
    () => localStorage.getItem("pf-preset") ?? "balanced",
  );
  const [presets, setPresets] = useState<
    { name: string; label: string; description: string }[]
  >([]);
  const [snapRadius, setSnapRadius] = useState(300);
  const [bridgeRadius, setBridgeRadius] = useState(3000);
  const [meshCell, setMeshCell] = useState(100);
  // Off-trail mesh padding — how far around the [from, to] bbox the
  // solver may detour. 0 = auto (≥ 30 % of route length, ≥ 400 m).
  // The endpoint-refusal snap radius accommodates clicks that land
  // just inside a sub-cell water sliver.
  const [meshPad, setMeshPad] = useState(0); // 0 = auto
  const [refusalSnap, setRefusalSnap] = useState(150);
  const [layerNames, setLayerNames] = useState<string[]>([]);
  const [layerWeights, setLayerWeights] = useState<Record<string, number>>({});
  // Per-request cost-config patch (Stage 3 SPA). Each entry is a
  // sparse override the curator twiddles against the boot config;
  // `null` means "inherit". Threaded into every pathfind/record/
  // stream request body as `prefs.cost_config_override` so the
  // server applies it to that one solve only. Boot-config defaults
  // are read from `/v1/debug/cost-config` so the sliders start at
  // the live value, not a guess.
  type CostConfigPatch = {
    off_trail_base_foot: number | null;
    off_trail_base_bicycle: number | null;
    off_trail_base_ski: number | null;
    trail_proximity_bonus_at_zero: number | null;
    trail_proximity_influence_radius_m: number | null;
    slope_cell_quadratic_scale_deg: number | null;
    slope_cell_refuse_above_deg: number | null;
    slope_graph_quadratic_scale_deg: number | null;
    slope_graph_refuse_above_deg: number | null;
    total_gain_amplifier: number | null;
  };
  const EMPTY_PATCH: CostConfigPatch = {
    off_trail_base_foot: null,
    off_trail_base_bicycle: null,
    off_trail_base_ski: null,
    trail_proximity_bonus_at_zero: null,
    trail_proximity_influence_radius_m: null,
    slope_cell_quadratic_scale_deg: null,
    slope_cell_refuse_above_deg: null,
    slope_graph_quadratic_scale_deg: null,
    slope_graph_refuse_above_deg: null,
    total_gain_amplifier: null,
  };
  const [costPatch, setCostPatch] = useState<CostConfigPatch>(EMPTY_PATCH);
  // Live boot config from `/v1/debug/cost-config`. Used to seed the
  // sliders with the server's actual defaults so unset rows show
  // the value the solver would use today, not a hardcoded literal.
  const [costConfigBoot, setCostConfigBoot] = useState<Record<string, number> | null>(null);
  const [costPanelOpen, setCostPanelOpen] = useState(false);
  const [path, setPath] = useState<PathfindResp | null>(null);
  // Algorithm-replay state. When `recordOn` is true the autopathfind
  // hits /v1/pathfind/record (forces Prefs.record=true), and the
  // resulting Path.recording animates on the map: explored set,
  // frontier, line-of-sight rays, emerging best path.
  const [recordOn, setRecordOn] = useState(false);
  // Live progress (best-path-so-far streaming) is the default experience.
  const [liveMode, setLiveMode] = useState(true);
  const [replayIdx, setReplayIdx] = useState(0);
  const [replayPlaying, setReplayPlaying] = useState(false);
  const [replaySpeed, setReplaySpeed] = useState(1.0);
  // Events collected over an SSE connection in live mode. When
  // recordOn && liveMode the autopathfind opens a fetch stream
  // to /pathfind/stream and pushes events into this array as they
  // arrive. The same overlays the record+replay path uses render
  // off this state, so the two modes share rendering code.
  const [liveEvents, setLiveEvents] = useState<
    import("../api/v1").SolverEvent[]
  >([]);
  const [liveDone, setLiveDone] = useState<boolean>(false);
  const liveAbortRef = useRef<AbortController | null>(null);
  // Live "trail being built". Driven by a requestAnimationFrame loop (not
  // React state) so the preview extends FLUIDLY: each best-path snapshot
  // sets `liveTargetRef`, and the loop eases the drawn length toward it,
  // rendering a solid round-capped line with a gentle pulsing glow.
  const liveTargetRef = useRef<[number, number][] | null>(null);
  const liveDrawnRef = useRef(0); // eased vertex count currently drawn
  const liveRafRef = useRef<number | null>(null);
  const livePhaseRef = useRef(0); // glow-pulse phase

  // Tear down the live preview (cancel the loop + remove its layers).
  const stopLivePreview = () => {
    if (liveRafRef.current != null) cancelAnimationFrame(liveRafRef.current);
    liveRafRef.current = null;
    liveTargetRef.current = null;
    liveDrawnRef.current = 0;
    const map = mapRef.current;
    if (map) {
      for (const id of ["live-best", "live-best-glow"]) {
        if (map.getLayer(id)) map.removeLayer(id);
      }
      if (map.getSource("live-best")) map.removeSource("live-best");
    }
  };
  // Start the rAF loop that eases the drawn length toward the latest
  // snapshot and breathes the glow, so the route extends fluidly.
  const startLivePreview = () => {
    if (liveRafRef.current != null) return; // already running
    liveDrawnRef.current = 0;
    const tick = () => {
      const map = mapRef.current;
      const target = liveTargetRef.current;
      if (!map || !target || target.length < 2) {
        liveRafRef.current = requestAnimationFrame(tick);
        return;
      }
      if (!map.getSource("live-best")) {
        const empty = { type: "Feature" as const, geometry: { type: "LineString" as const, coordinates: [] as [number, number][] }, properties: {} };
        map.addSource("live-best", { type: "geojson", data: empty });
        map.addLayer({
          id: "live-best-glow", type: "line", source: "live-best",
          layout: { "line-cap": "round", "line-join": "round" },
          paint: { "line-color": CVD_BLUE, "line-width": 13, "line-opacity": 0.18, "line-blur": 4 },
        });
        map.addLayer({
          id: "live-best", type: "line", source: "live-best",
          layout: { "line-cap": "round", "line-join": "round" },
          paint: { "line-color": CVD_BLUE, "line-width": 4, "line-opacity": 0.95 },
        });
      }
      // Exponential ease of the visible vertex count toward the target.
      const tgt = target.length;
      let d = liveDrawnRef.current < 2 ? 2 : liveDrawnRef.current;
      d += (tgt - d) * 0.22;
      if (tgt - d < 0.6) d = tgt;
      liveDrawnRef.current = d;
      const coords = target.slice(0, Math.max(2, Math.round(d)));
      const src = map.getSource("live-best") as maplibregl.GeoJSONSource | undefined;
      src?.setData({ type: "Feature", geometry: { type: "LineString", coordinates: coords }, properties: {} });
      // Gentle breathing glow so it feels alive between snapshots.
      livePhaseRef.current += 0.09;
      if (map.getLayer("live-best-glow")) {
        map.setPaintProperty(
          "live-best-glow",
          "line-opacity",
          0.14 + 0.1 * (0.5 + 0.5 * Math.sin(livePhaseRef.current)),
        );
      }
      liveRafRef.current = requestAnimationFrame(tick);
    };
    liveRafRef.current = requestAnimationFrame(tick);
  };
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [demCoverage, setDemCoverage] = useState<DemCoverage | null>(null);
  const [maskCoverage, setMaskCoverage] = useState<MaskCoverage | null>(null);
  const [graphStats, setGraphStats] = useState<GraphStats | null>(null);
  const [graphDensity, setGraphDensity] = useState<GraphDensity | null>(null);
  // Coverage extents + graph-density dots are developer diagnostics —
  // off by default so the planner map is clean. Toggle them on in the
  // Advanced (testing) drawer when debugging data coverage.
  const [showCoverage, setShowCoverage] = useState(false);
  const [showDensity, setShowDensity] = useState(false);
  const [basemap, setBasemap] = useState<BasemapId>("topo");
  /// Set when the latest pathfind failed with HTTP 422; carries the
  /// machine-readable hint so we can recolour markers + tell the
  /// user which click landed in a refused region.
  const [refusal, setRefusal] = useState<{ which: "from" | "to"; layer: string } | null>(null);
  const [showMesh, setShowMesh] = useState(false);
  const [inspect, setInspect] = useState<InspectResp | null>(null);

  // Viewport-bbox data overlays. Every primitive layer can be toggled
  // independently so a curator can see exactly which data is driving
  // pathfinding decisions in the current view. Each fetch runs on
  // map idle (after pan/zoom) for the visible viewport with a hard
  // cap on the number of features returned.
  const [showWater, setShowWater] = useState(false);
  const [showWetland, setShowWetland] = useState(false);
  const [showForest, setShowForest] = useState(false);
  const [showEdgesSti, setShowEdgesSti] = useState(false);
  const [showEdgesVei, setShowEdgesVei] = useState(false);
  const [showEdgesSki, setShowEdgesSki] = useState(false);
  const [showAnchors, setShowAnchors] = useState(false);

  const [waterCells, setWaterCells] = useState<MaskCellsResp | null>(null);
  const [wetlandCells, setWetlandCells] = useState<MaskCellsResp | null>(null);
  const [forestCells, setForestCells] = useState<MaskCellsResp | null>(null);
  const [stiEdges, setStiEdges] = useState<EdgesResp | null>(null);
  const [veiEdges, setVeiEdges] = useState<EdgesResp | null>(null);
  const [skiEdges, setSkiEdges] = useState<EdgesResp | null>(null);
  const [anchorPts, setAnchorPts] = useState<AnchorsBboxResp | null>(null);
  // Bumps every time the map settles after pan/zoom, triggering
  // a refetch of whichever overlays are on.
  const [viewportTick, setViewportTick] = useState(0);
  // Click-to-inspect mode flips map-click semantics. While on, the
  // click queries `/v1/debug/pathfind/cell` instead of placing a
  // marker — so curators can probe arbitrary points without
  // disturbing the active route.
  const [inspectMode, setInspectMode] = useState(false);
  const [cellInfo, setCellInfo] = useState<CellInspectResp | null>(null);
  // When set, skip strategies 1 + 2 and go straight to off-trail.
  // Useful when the graph topology is too sparse to trust — forces
  // the solver to find a path via mesh + Theta* only.
  const [forceOffTrail, setForceOffTrail] = useState(false);

  // Clean-UI state. The side panel defaults to a minimal hiker view;
  // every developer/calibration control lives inside the collapsed
  // "Advanced (testing)" drawer below.
  const [advancedOpen, setAdvancedOpen] = useState(false);
  // Stops list collapsed/expanded (keeps the sheet short with many stops).
  const [stopsOpen, setStopsOpen] = useState(true);
  // Right-hand debug/layers pane — collapsed by default so it stays out
  // of the way; a small icon button reopens it.
  const [debugOpen, setDebugOpen] = useState(false);
  // Total ascent (metres) of the current route, computed from a DEM
  // elevation profile of the geometry. `null` until fetched. Drives
  // the hiker-friendly result card + the Naismith time estimate.
  const [gainM, setGainM] = useState<number | null>(null);

  // The click handler is installed once at mount; React state read
  // inside that handler would be stale forever. Mirror everything
  // it needs into refs so each click sees the latest state.
  const fromRef = useRef<Marker | null>(null);
  const toRef = useRef<Marker | null>(null);
  const inspectModeRef = useRef(false);
  fromRef.current = from;
  toRef.current = to;
  inspectModeRef.current = inspectMode;
  // Current route, for the once-bound drag-to-insert handler to read.
  const pathRef = useRef<PathfindResp | null>(null);
  pathRef.current = path;

  // Load trip-style presets once; persist the selection.
  useEffect(() => {
    v1.get<{ name: string; label: string; description: string }[]>("/route/presets")
      .then(setPresets)
      .catch(() => setPresets([]));
  }, []);
  useEffect(() => {
    localStorage.setItem("pf-preset", preset);
  }, [preset]);

  // Keep the derived endpoints in sync with the waypoint list so the
  // endpoint-keyed overlays (inspect, mesh, coverage) follow the start
  // and end stops. The full `points` array drives the route request.
  useEffect(() => {
    setFrom(points[0] ?? null);
    setTo(points.length >= 2 ? points[points.length - 1] : null);
  }, [points]);

  useEffect(() => {
    v1.get<LayersResp>("/debug/pathfind/layers")
      .then((r) => {
        setLayerNames(r.layers);
        const init: Record<string, number> = {};
        for (const n of r.layers) init[n] = 1.0;
        setLayerWeights(init);
      })
      .catch(() => {
        // No tileserver? Leave panel empty; the map still works.
      });
    // DEM coverage is optional; the admin probe handles 503 by
    // simply not rendering the DEM overlay.
    v1.get<DemCoverage>("/debug/elev/coverage")
      .then(setDemCoverage)
      .catch(() => {});
    v1.get<MaskCoverage>("/debug/mask/coverage")
      .then(setMaskCoverage)
      .catch(() => {});
    v1.get<GraphStats>("/debug/graph/stats")
      .then(setGraphStats)
      .catch(() => {});
    v1.get<GraphDensity>("/debug/graph/density")
      .then(setGraphDensity)
      .catch(() => {});
    // Pull the boot cost config so the calibration panel's sliders
    // can show the value the solver would use today when the curator
    // hasn't overridden it. The endpoint returns the resolved boot
    // config (file → env → CWD → embedded fallback).
    v1.get<{
      off_trail_base: { foot: number; bicycle: number; ski: number };
      trail_proximity: { bonus_at_zero: number; influence_radius_m: number };
      slope_cell: { quadratic_scale_deg: number; refuse_above_deg: number };
      slope_graph: { quadratic_scale_deg: number; refuse_above_deg: number };
      total_gain: { amplifier: number };
    }>("/debug/cost-config")
      .then((c) => {
        setCostConfigBoot({
          off_trail_base_foot: c.off_trail_base.foot,
          off_trail_base_bicycle: c.off_trail_base.bicycle,
          off_trail_base_ski: c.off_trail_base.ski,
          trail_proximity_bonus_at_zero: c.trail_proximity.bonus_at_zero,
          trail_proximity_influence_radius_m: c.trail_proximity.influence_radius_m,
          slope_cell_quadratic_scale_deg: c.slope_cell.quadratic_scale_deg,
          slope_cell_refuse_above_deg: c.slope_cell.refuse_above_deg,
          slope_graph_quadratic_scale_deg: c.slope_graph.quadratic_scale_deg,
          slope_graph_refuse_above_deg: c.slope_graph.refuse_above_deg,
          total_gain_amplifier: c.total_gain.amplifier,
        });
      })
      .catch(() => {});
  }, []);

  // Init map once. Basemap-swapping is handled by a separate effect
  // below that re-points the `basemap` source — cheaper than
  // rebuilding the whole style.
  useEffect(() => {
    if (!containerRef.current || mapRef.current) return;
    const initial = BASEMAPS[basemap];
    const map = new maplibregl.Map({
      container: containerRef.current,
      style: {
        version: 8,
        sources: {
          basemap: {
            type: "raster",
            tiles: [...initial.tiles],
            tileSize: 256,
            attribution: initial.attribution,
          },
        },
        layers: [{ id: "basemap", type: "raster", source: "basemap" }],
      },
      center: FALLBACK_CENTER,
      zoom: FALLBACK_ZOOM,
    });
    mapRef.current = map;
    map.addControl(new maplibregl.NavigationControl(), "top-right");
    map.addControl(new maplibregl.ScaleControl({ unit: "metric" }), "bottom-left");
    // Escape hatch for headless smoke tests: expose the map instance
    // + bounded state setters on `window`. No security implication —
    // a user clicking on the map already drives `setFrom`/`setTo` via
    // the click handler below. This is the same surface, exposed for
    // automation. Curators don't touch it.
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (window as any).__pf = {
      map,
      // Append a waypoint (the new end). Mirrors a map click.
      addPoint: (lon: number, lat: number) =>
        commitPoints([...pointsRef.current, [lon, lat]]),
      setPoints: (pts: [number, number][]) => commitPoints(pts),
      // Back-compat with existing smoke scripts: setFrom resets to a
      // single start; setTo appends an end.
      setFrom: (lon: number, lat: number) => commitPoints([[lon, lat]]),
      setTo: (lon: number, lat: number) =>
        commitPoints([...pointsRef.current.slice(0, 1), [lon, lat]]),
      setForceOffTrail: (v: boolean) => setForceOffTrail(v),
      reset: () => commitPoints([]),
    };

    // Each time the map settles after pan/zoom, bump the viewport
    // tick — the per-overlay effects use it as their dependency so
    // they refetch their data for the new visible bbox.
    map.on("idle", () => setViewportTick((n) => n + 1));

    // Click-to-place when in normal mode; click-to-inspect when the
    // inspect toggle is on. Inspect mode never disturbs from/to.
    map.on("click", (e) => {
      const lonlat: Marker = [e.lngLat.lng, e.lngLat.lat];
      if (inspectModeRef.current) {
        v1.post<CellInspectResp>("/debug/pathfind/cell", {
          lon: lonlat[0],
          lat: lonlat[1],
        })
          .then(setCellInfo)
          .catch(() => setCellInfo(null));
        return;
      }
      setErr(null);
      // Append: each click adds a stop; the newest click is the new
      // destination, earlier ones become vias.
      commitPoints([...pointsRef.current, lonlat]);
    });

    // Drag-the-route-to-insert (the Komoot/Google gesture). Grab the
    // invisible `path-hit` line anywhere and drag; on release a new stop
    // is inserted into the leg that was grabbed. Bound once; reads the
    // current route via pathRef so it stays correct as routes change.
    const insertIndexFor = (lng: number, lat: number): number | null => {
      const p = pathRef.current?.path;
      if (!p || p.geometry.length < 2) return null;
      let best = 0;
      let bestD = Infinity;
      for (let k = 0; k < p.geometry.length; k++) {
        const dx = p.geometry[k][0] - lng;
        const dy = p.geometry[k][1] - lat;
        const d = dx * dx + dy * dy;
        if (d < bestD) { bestD = d; best = k; }
      }
      const legs = p.waypoint_legs ?? [];
      const leg = legs.find(
        (l) => best >= l.geometry_start_idx && best <= l.geometry_end_idx,
      );
      // Insert between the grabbed leg's endpoints; fall back to before
      // the final stop if leg metadata is missing.
      return leg ? leg.from_point_idx + 1 : Math.max(1, pointsRef.current.length - 1);
    };
    let provisional: maplibregl.Marker | null = null;
    // Cursor affordance: show "copy" when hovering the route.
    map.on("mousemove", (e) => {
      if (provisional) return;
      const over = map.getLayer("path-hit")
        ? map.queryRenderedFeatures(e.point, { layers: ["path-hit"] }).length > 0
        : false;
      map.getCanvas().style.cursor = over ? "copy" : "";
    });
    // General mousedown + hit-test (layer-scoped mousedown doesn't fire
    // reliably here). If the press landed on the route, start an insert.
    map.on("mousedown", (e) => {
      if (!map.getLayer("path-hit")) return;
      if (map.queryRenderedFeatures(e.point, { layers: ["path-hit"] }).length === 0) return;
      e.preventDefault(); // suppress map pan while inserting
      map.dragPan.disable();
      provisional = new maplibregl.Marker({ color: "#374151" })
        .setLngLat(e.lngLat)
        .addTo(map);
      const onMove = (ev: maplibregl.MapMouseEvent) =>
        provisional?.setLngLat(ev.lngLat);
      const onUp = (ev: maplibregl.MapMouseEvent) => {
        map.off("mousemove", onMove);
        map.dragPan.enable();
        map.getCanvas().style.cursor = "";
        provisional?.remove();
        provisional = null;
        const at = insertIndexFor(ev.lngLat.lng, ev.lngLat.lat);
        if (at != null) {
          const next = [...pointsRef.current];
          next.splice(at, 0, [ev.lngLat.lng, ev.lngLat.lat]);
          commitPoints(next);
        }
      };
      map.on("mousemove", onMove);
      map.once("mouseup", onUp);
    });

    return () => {
      map.remove();
      mapRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Re-point the basemap source when the user picks a different
  // basemap. Style swap would also work but is heavier — replacing
  // tile URLs on the existing source keeps state (path layers,
  // markers, viewport) intact.
  useEffect(() => {
    const map = mapRef.current;
    if (!map) return;
    const apply = () => {
      const next = BASEMAPS[basemap];
      // The MapLibre type for RasterTileSource exposes `setTiles`.
      const src = map.getSource("basemap") as maplibregl.RasterTileSource | undefined;
      if (src && typeof src.setTiles === "function") {
        src.setTiles([...next.tiles]);
      } else {
        // Fallback: rebuild the source entirely.
        if (map.getLayer("basemap")) map.removeLayer("basemap");
        if (map.getSource("basemap")) map.removeSource("basemap");
        map.addSource("basemap", {
          type: "raster",
          tiles: [...next.tiles],
          tileSize: 256,
          attribution: next.attribution,
        });
        map.addLayer({ id: "basemap", type: "raster", source: "basemap" }, undefined);
      }
    };
    if (map.isStyleLoaded()) apply();
    else map.once("load", apply);
  }, [basemap]);

  // Centre on whichever coverage we have. Prefer graph (it's the
  // most useful for path queries); fall back to DEM, then mask.
  useEffect(() => {
    const map = mapRef.current;
    if (!map) return;
    const utmToLonLat = (x: number, y: number): [number, number] => {
      // Inverse of the crude approximation used in older versions —
      // accurate to ~100 m which is plenty for fly-to.
      const lon = 15.0 + (x - 500000) / 111320 / Math.cos((60.0 * Math.PI) / 180);
      const lat = y / 111320;
      return [lon, lat];
    };
    const bboxFromGraph = graphStats
      ? [
          ...utmToLonLat(graphStats.min_x, graphStats.min_y),
          ...utmToLonLat(graphStats.max_x, graphStats.max_y),
        ]
      : null;
    const bboxFromDem = demCoverage
      ? [
          ...utmToLonLat(demCoverage.min_x, demCoverage.min_y),
          ...utmToLonLat(demCoverage.max_x, demCoverage.max_y),
        ]
      : null;
    const bboxFromMask = maskCoverage
      ? [
          ...utmToLonLat(maskCoverage.meta.min_x, maskCoverage.meta.min_y),
          ...utmToLonLat(maskCoverage.meta.max_x, maskCoverage.meta.max_y),
        ]
      : null;
    const bbox = bboxFromGraph ?? bboxFromDem ?? bboxFromMask;
    if (!bbox) return;
    map.fitBounds([[bbox[0], bbox[1]], [bbox[2], bbox[3]]], {
      padding: 60,
      maxZoom: 13,
      duration: 600,
    });
  }, [graphStats, demCoverage, maskCoverage]);

  // Draw coverage overlays. Three sources can render rectangles —
  // each in a different colour and translucency so the layering is
  // legible.
  useEffect(() => {
    const map = mapRef.current;
    if (!map) return;
    const utmToLonLat = (x: number, y: number): [number, number] => {
      const lon = 15.0 + (x - 500000) / 111320 / Math.cos((60.0 * Math.PI) / 180);
      const lat = y / 111320;
      return [lon, lat];
    };

    const apply = () => {
      for (const id of ["coverage-dem", "coverage-mask", "coverage-graph"]) {
        if (map.getLayer(id)) map.removeLayer(id);
        if (map.getLayer(`${id}-outline`)) map.removeLayer(`${id}-outline`);
        if (map.getSource(id)) map.removeSource(id);
      }
      if (!showCoverage) return;
      const addRect = (
        id: string,
        bbox: [number, number, number, number] | null,
        fill: string,
        outline: string,
        opacity: number,
      ) => {
        if (!bbox) return;
        const [w, s, e, n] = bbox;
        map.addSource(id, {
          type: "geojson",
          data: {
            type: "Feature",
            geometry: {
              type: "Polygon",
              coordinates: [[[w, s], [e, s], [e, n], [w, n], [w, s]]],
            },
            properties: {},
          },
        });
        map.addLayer({
          id,
          type: "fill",
          source: id,
          paint: { "fill-color": fill, "fill-opacity": opacity },
        });
        map.addLayer({
          id: `${id}-outline`,
          type: "line",
          source: id,
          paint: { "line-color": outline, "line-width": 1.5, "line-dasharray": [4, 3] },
        });
      };
      if (demCoverage) {
        const w = utmToLonLat(demCoverage.min_x, demCoverage.min_y);
        const e = utmToLonLat(demCoverage.max_x, demCoverage.max_y);
        addRect("coverage-dem", [w[0], w[1], e[0], e[1]], "#0ea5e9", "#0369a1", 0.05);
      }
      if (maskCoverage) {
        const w = utmToLonLat(maskCoverage.meta.min_x, maskCoverage.meta.min_y);
        const e = utmToLonLat(maskCoverage.meta.max_x, maskCoverage.meta.max_y);
        addRect("coverage-mask", [w[0], w[1], e[0], e[1]], "#22c55e", "#15803d", 0.04);
      }
      if (graphStats) {
        const w = utmToLonLat(graphStats.min_x, graphStats.min_y);
        const e = utmToLonLat(graphStats.max_x, graphStats.max_y);
        addRect("coverage-graph", [w[0], w[1], e[0], e[1]], "#f59e0b", "#b45309", 0.04);
      }
    };
    if (map.isStyleLoaded()) apply();
    else map.once("load", apply);
  }, [demCoverage, maskCoverage, graphStats, showCoverage]);

  // Helper: read the current map viewport as a bbox query string.
  // Returns null when the map isn't initialised yet.
  const viewportBboxQs = (limit: number, filter?: string): string | null => {
    const map = mapRef.current;
    if (!map) return null;
    const b = map.getBounds();
    let qs =
      `west=${b.getWest().toFixed(5)}&south=${b.getSouth().toFixed(5)}` +
      `&east=${b.getEast().toFixed(5)}&north=${b.getNorth().toFixed(5)}` +
      `&limit=${limit}`;
    if (filter) qs += `&filter=${encodeURIComponent(filter)}`;
    return qs;
  };

  // Fetch mask-style cell overlays when toggled on or viewport moves.
  useEffect(() => {
    if (!showWater) {
      setWaterCells(null);
      return;
    }
    const qs = viewportBboxQs(8000);
    if (!qs) return;
    let cancel = false;
    v1.get<MaskCellsResp>(`/debug/data/water?${qs}`)
      .then((r) => { if (!cancel) setWaterCells(r); })
      .catch(() => { if (!cancel) setWaterCells(null); });
    return () => { cancel = true; };
  }, [showWater, viewportTick]);
  useEffect(() => {
    if (!showWetland) {
      setWetlandCells(null);
      return;
    }
    const qs = viewportBboxQs(8000);
    if (!qs) return;
    let cancel = false;
    v1.get<MaskCellsResp>(`/debug/data/wetland?${qs}`)
      .then((r) => { if (!cancel) setWetlandCells(r); })
      .catch(() => { if (!cancel) setWetlandCells(null); });
    return () => { cancel = true; };
  }, [showWetland, viewportTick]);
  useEffect(() => {
    if (!showForest) {
      setForestCells(null);
      return;
    }
    const qs = viewportBboxQs(8000);
    if (!qs) return;
    let cancel = false;
    v1.get<MaskCellsResp>(`/debug/data/forest?${qs}`)
      .then((r) => { if (!cancel) setForestCells(r); })
      .catch(() => { if (!cancel) setForestCells(null); });
    return () => { cancel = true; };
  }, [showForest, viewportTick]);

  // Edge fetches — per-fkb_type.
  useEffect(() => {
    if (!showEdgesSti) {
      setStiEdges(null);
      return;
    }
    const qs = viewportBboxQs(4000, "sti");
    if (!qs) return;
    let cancel = false;
    v1.get<EdgesResp>(`/debug/data/edges?${qs}`)
      .then((r) => { if (!cancel) setStiEdges(r); })
      .catch(() => { if (!cancel) setStiEdges(null); });
    return () => { cancel = true; };
  }, [showEdgesSti, viewportTick]);
  useEffect(() => {
    if (!showEdgesVei) {
      setVeiEdges(null);
      return;
    }
    const qs = viewportBboxQs(4000, "vei");
    if (!qs) return;
    let cancel = false;
    v1.get<EdgesResp>(`/debug/data/edges?${qs}`)
      .then((r) => { if (!cancel) setVeiEdges(r); })
      .catch(() => { if (!cancel) setVeiEdges(null); });
    return () => { cancel = true; };
  }, [showEdgesVei, viewportTick]);
  useEffect(() => {
    if (!showEdgesSki) {
      setSkiEdges(null);
      return;
    }
    const qs = viewportBboxQs(4000, "skiloype");
    if (!qs) return;
    let cancel = false;
    v1.get<EdgesResp>(`/debug/data/edges?${qs}`)
      .then((r) => { if (!cancel) setSkiEdges(r); })
      .catch(() => { if (!cancel) setSkiEdges(null); });
    return () => { cancel = true; };
  }, [showEdgesSki, viewportTick]);

  useEffect(() => {
    if (!showAnchors) {
      setAnchorPts(null);
      return;
    }
    const qs = viewportBboxQs(1500);
    if (!qs) return;
    let cancel = false;
    v1.get<AnchorsBboxResp>(`/debug/data/anchors?${qs}`)
      .then((r) => { if (!cancel) setAnchorPts(r); })
      .catch(() => { if (!cancel) setAnchorPts(null); });
    return () => { cancel = true; };
  }, [showAnchors, viewportTick]);

  // Render a mask-cells overlay. Helper used by water/wetland/forest.
  const renderMaskCells = (
    id: string,
    data: MaskCellsResp | null,
    color: string,
  ) => {
    const map = mapRef.current;
    if (!map) return;
    const apply = () => {
      if (map.getLayer(id)) map.removeLayer(id);
      if (map.getSource(id)) map.removeSource(id);
      if (!data || data.cells.length === 0) return;
      map.addSource(id, {
        type: "geojson",
        data: {
          type: "FeatureCollection",
          features: data.cells.map((c) => ({
            type: "Feature" as const,
            properties: {},
            geometry: { type: "Point" as const, coordinates: [c[0], c[1]] },
          })),
        },
      });
      map.addLayer({
        id,
        type: "circle",
        source: id,
        paint: {
          "circle-color": color,
          "circle-radius": 3,
          "circle-opacity": 0.45,
        },
      });
    };
    if (map.isStyleLoaded()) apply();
    else map.once("load", apply);
  };

  useEffect(() => {
    renderMaskCells("overlay-water", waterCells, "#1e3a8a"); // navy
  }, [waterCells]);
  useEffect(() => {
    renderMaskCells("overlay-wetland", wetlandCells, "#7c3aed"); // violet
  }, [wetlandCells]);
  useEffect(() => {
    renderMaskCells("overlay-forest", forestCells, "#15803d"); // forest green
  }, [forestCells]);

  // Edge overlays. Different colours per surface so curators can
  // distinguish trails / roads / ski tracks at a glance.
  const renderEdges = (
    id: string,
    data: EdgesResp | null,
    color: string,
    width: number,
    dash: number[] | null,
  ) => {
    const map = mapRef.current;
    if (!map) return;
    const apply = () => {
      if (map.getLayer(id)) map.removeLayer(id);
      if (map.getSource(id)) map.removeSource(id);
      if (!data || data.edges.length === 0) return;
      map.addSource(id, {
        type: "geojson",
        data: {
          type: "FeatureCollection",
          features: data.edges.map((e) => ({
            type: "Feature" as const,
            properties: { kind: e.kind },
            geometry: {
              type: "LineString" as const,
              coordinates: e.coords,
            },
          })),
        },
      });
      const paint: Record<string, unknown> = {
        "line-color": color,
        "line-width": width,
        "line-opacity": 0.7,
      };
      if (dash) paint["line-dasharray"] = dash;
      map.addLayer({ id, type: "line", source: id, paint });
    };
    if (map.isStyleLoaded()) apply();
    else map.once("load", apply);
  };

  useEffect(() => {
    renderEdges("overlay-sti", stiEdges, "#dc2626", 2.5, null); // red
  }, [stiEdges]);
  useEffect(() => {
    renderEdges("overlay-vei", veiEdges, "#374151", 2, [3, 1]); // grey dashed
  }, [veiEdges]);
  useEffect(() => {
    renderEdges("overlay-ski", skiEdges, "#0ea5e9", 2.5, [4, 2]); // cyan dashed
  }, [skiEdges]);

  // Anchor overlay — small coloured dot per anchor with hover popup.
  useEffect(() => {
    const map = mapRef.current;
    if (!map) return;
    const apply = () => {
      if (map.getLayer("overlay-anchors")) map.removeLayer("overlay-anchors");
      if (map.getSource("overlay-anchors")) map.removeSource("overlay-anchors");
      if (!anchorPts || anchorPts.anchors.length === 0) return;
      map.addSource("overlay-anchors", {
        type: "geojson",
        data: {
          type: "FeatureCollection",
          features: anchorPts.anchors.map((a) => ({
            type: "Feature" as const,
            properties: { kind: a.kind, name: a.name ?? "" },
            geometry: { type: "Point" as const, coordinates: [a.lon, a.lat] },
          })),
        },
      });
      map.addLayer({
        id: "overlay-anchors",
        type: "circle",
        source: "overlay-anchors",
        paint: {
          "circle-color": [
            "match", ["get", "kind"],
            "summit", "#dc2626",
            "cabin", "#a16207",
            "waterfeature", "#0ea5e9",
            "viewpoint", "#7c3aed",
            "trailhead", "#16a34a",
            "parking", "#374151",
            /* default */ "#9ca3af",
          ],
          "circle-radius": 5,
          "circle-stroke-color": "#fff",
          "circle-stroke-width": 1.2,
        },
      });
    };
    if (map.isStyleLoaded()) apply();
    else map.once("load", apply);
  }, [anchorPts]);

  // Graph density: sampled node dots so curators can see WHERE the
  // trail network actually has data — coverage rectangles don't
  // distinguish a dense Lofoten patch from a sparse fjord scribble.
  useEffect(() => {
    const map = mapRef.current;
    if (!map || !graphDensity) return;
    const apply = () => {
      if (map.getLayer("graph-density")) map.removeLayer("graph-density");
      if (map.getSource("graph-density")) map.removeSource("graph-density");
      if (!showDensity) return;
      map.addSource("graph-density", {
        type: "geojson",
        data: {
          type: "FeatureCollection",
          features: graphDensity.points.map((p) => ({
            type: "Feature",
            geometry: { type: "Point", coordinates: p },
            properties: {},
          })),
        },
      });
      map.addLayer({
        id: "graph-density",
        type: "circle",
        source: "graph-density",
        paint: {
          "circle-color": "#b45309",
          "circle-radius": 2,
          "circle-opacity": 0.45,
        },
      });
    };
    if (map.isStyleLoaded()) apply();
    else map.once("load", apply);
  }, [graphDensity, showDensity]);

  // Markers: render EVERY waypoint as a numbered, draggable pin.
  // Order is encoded by LABEL + SHAPE, never colour alone (the curator
  // is colour-blind): start = "S" (blue circle), end = "E" (vermillion
  // square), vias = their number (grey circle). A refused endpoint gets
  // a red ring. Drag a pin to move that stop; double-click to remove it.
  useEffect(() => {
    const map = mapRef.current;
    if (!map) return;
    for (const m of waypointMarkersRef.current) m.remove();
    waypointMarkersRef.current = [];
    points.forEach((pt, i) => {
      const isStart = i === 0;
      const isEnd = i === points.length - 1 && points.length >= 2;
      const refused =
        (refusal?.which === "from" && isStart) ||
        (refusal?.which === "to" && isEnd);
      const label = isStart ? "S" : isEnd ? "E" : String(i);
      const bg = refused
        ? "#9ca3af"
        : isStart
          ? CVD_BLUE
          : isEnd
            ? CVD_VERMILLION
            : "#374151";
      const el = document.createElement("div");
      el.setAttribute("data-wp", String(i));
      el.title = isStart ? "Start" : isEnd ? "End" : `Stop ${i}`;
      el.textContent = label;
      el.style.cssText = [
        "width:24px",
        "height:24px",
        // End is a rounded square so start/end differ by SHAPE too.
        isEnd ? "border-radius:5px" : "border-radius:9999px",
        `background:${bg}`,
        "color:#fff",
        "font:600 12px/22px system-ui,sans-serif",
        "text-align:center",
        "box-shadow:0 1px 3px rgba(0,0,0,.45)",
        `border:2px solid ${refused ? "#dc2626" : "#fff"}`,
        "cursor:grab",
      ].join(";");
      const marker = new maplibregl.Marker({
        element: el,
        draggable: true,
        anchor: "center",
      })
        .setLngLat(pt)
        .addTo(map);
      marker.on("dragend", () => {
        const ll = marker.getLngLat();
        commitPoints(
          pointsRef.current.map((p, idx) => (idx === i ? [ll.lng, ll.lat] : p)),
        );
      });
      el.addEventListener("dblclick", (ev) => {
        ev.stopPropagation();
        commitPoints(pointsRef.current.filter((_, idx) => idx !== i));
      });
      waypointMarkersRef.current.push(marker);
    });
  }, [points, refusal]);

  // Auto-pathfind when there are >= 2 waypoints or any control changes.
  useEffect(() => {
    if (points.length < 2) {
      setPath(null);
      stopLivePreview();
      return;
    }
    let cancelled = false;
    setBusy(true);
    setErr(null);
    setRefusal(null);
    // Three endpoints share the same request shape:
    //   /pathfind         — plain solve, no recording.
    //   /pathfind/record  — solve with full per-event recording
    //                       embedded in the response.
    //   /pathfind/stream  — SSE; events flow as the solver runs.
    // The SPA picks the right one based on the recordOn + liveMode
    // toggles. Live mode is handled via a separate fetch + reader
    // below so this v1.post path only handles the first two.
    if (recordOn && liveMode) {
      // Cancel any in-flight live stream before starting a new one.
      if (liveAbortRef.current) liveAbortRef.current.abort();
      const ac = new AbortController();
      liveAbortRef.current = ac;
      setLiveEvents([]);
      setLiveDone(false);
      setPath(null);
      // Kick off an async reader; don't await inside the effect.
      const overrideForReq = nonNullPatch(costPatch);
      void streamPathfind(points, preset, {
        profile,
        snap_radius_m: forceOffTrail ? 0 : snapRadius,
        bridge_radius_m: forceOffTrail ? 0 : bridgeRadius,
        mesh_cell_m: meshCell,
        mesh_pad_m: meshPad > 0 ? meshPad : null,
        refusal_snap_m: refusalSnap,
        max_off_trail_km: 20,
        allow_off_trail: true,
        layer_weights: layerWeights,
        ...(overrideForReq ? { cost_config_override: overrideForReq } : {}),
      }, ac.signal, {
        onSolver: (ev) => {
          setLiveEvents((prev) => {
            // Cap to keep the in-memory list bounded on huge solves.
            // 50K events is enough to see the full Marka exploration;
            // beyond that the SSE keeps coming but we render the
            // most recent slice.
            if (prev.length >= 50_000) return prev;
            return [...prev, ev];
          });
        },
        onDone: (resp) => {
          setPath({ path: resp, took_us: 0, layers: [] });
          setBusy(false);
          setLiveDone(true);
        },
        onError: (msg) => {
          setErr(msg);
          setBusy(false);
          setLiveDone(true);
        },
      });
      return;
    }
    // Default plotting streams the solve so the user watches the route
    // build live (the solver emits best-path-so-far snapshots as it runs).
    // The full record+replay path above is for the Advanced panel; here we
    // just track the latest snapshot and swap in the final route on `done`.
    if (liveAbortRef.current) liveAbortRef.current.abort();
    const ac = new AbortController();
    liveAbortRef.current = ac;
    stopLivePreview();
    setPath(null);
    const overrideForReq = nonNullPatch(costPatch);
    void streamPathfind(points, preset, {
      profile,
      snap_radius_m: forceOffTrail ? 0 : snapRadius,
      bridge_radius_m: forceOffTrail ? 0 : bridgeRadius,
      mesh_cell_m: meshCell,
      mesh_pad_m: meshPad > 0 ? meshPad : null,
      refusal_snap_m: refusalSnap,
      max_off_trail_km: 20,
      allow_off_trail: true,
      layer_weights: layerWeights,
      ...(overrideForReq ? { cost_config_override: overrideForReq } : {}),
    }, ac.signal, {
      onSolver: (ev) => {
        if (ev.kind === "best_path_snapshot") {
          liveTargetRef.current = ev.coords;
          startLivePreview();
        }
      },
      onDone: (resp) => {
        if (cancelled) return;
        setPath({ path: resp, took_us: 0, layers: [] });
        stopLivePreview();
        setBusy(false);
      },
      onError: (msg) => {
        if (cancelled) return;
        // Streamed errors arrive as a message string. Endpoint-refusal
        // (a click in a lake/glacier) is surfaced as a friendly refusal.
        const lower = msg.toLowerCase();
        if (lower.includes("refus")) {
          const which = lower.includes("to ") || lower.includes("destination") ? "to" : "from";
          setRefusal({ which, layer: "mask_refusal" });
        }
        setErr(msg);
        stopLivePreview();
        setBusy(false);
      },
    });
    return () => {
      ac.abort();
      cancelled = true;
      stopLivePreview();
    };
  }, [points, preset, profile, snapRadius, bridgeRadius, meshCell, meshPad, refusalSnap, layerWeights, forceOffTrail, recordOn, liveMode, costPatch]);

  // Inspect overlay: fetch mesh + refused regions when the user
  // toggles "Show mesh" and we have both markers. Refetches when
  // the route, layer weights, padding, or mesh size change so the
  // overlay always matches the latest solve.
  useEffect(() => {
    if (!showMesh || !from || !to) {
      setInspect(null);
      return;
    }
    let cancelled = false;
    v1.post<InspectResp>("/debug/pathfind/inspect", {
      from,
      to,
      prefs: {
        profile,
        mesh_cell_m: meshCell,
        mesh_pad_m: meshPad > 0 ? meshPad : null,
        layer_weights: layerWeights,
      },
    })
      .then((r) => {
        if (!cancelled) setInspect(r);
      })
      .catch(() => {
        if (!cancelled) setInspect(null);
      });
    return () => {
      cancelled = true;
    };
  }, [showMesh, from, to, profile, meshCell, meshPad, layerWeights]);

  // Draw the mesh + refused polygons overlay when inspect data is
  // available. Cells are coloured by cost_mul on a heat scale; refused
  // polygons are translucent red so the user can see exactly which
  // cells were vetoed.
  useEffect(() => {
    const map = mapRef.current;
    if (!map) return;
    const apply = () => {
      for (const id of ["mesh-cells", "mesh-refused", "mesh-refused-outline"]) {
        if (map.getLayer(id)) map.removeLayer(id);
      }
      for (const id of ["mesh-cells", "mesh-refused"]) {
        if (map.getSource(id)) map.removeSource(id);
      }
      if (!inspect || !showMesh) return;
      // Cells as half-cell-wide squares around their centres. Cost
      // multiplier → colour: 1.0×=cyan, 2×=yellow, 3×+=red.
      const half = inspect.inspect.mesh_cell_m * 0.5;
      const meters_per_deg_lat = 111320;
      const cells_geom = inspect.inspect.cells.map((c) => {
        const dlat = half / meters_per_deg_lat;
        const dlon = half / (meters_per_deg_lat * Math.cos((c.lat * Math.PI) / 180));
        return {
          type: "Feature" as const,
          properties: { cost: c.cost_mul },
          geometry: {
            type: "Polygon" as const,
            coordinates: [[
              [c.lon - dlon, c.lat - dlat],
              [c.lon + dlon, c.lat - dlat],
              [c.lon + dlon, c.lat + dlat],
              [c.lon - dlon, c.lat + dlat],
              [c.lon - dlon, c.lat - dlat],
            ]],
          },
        };
      });
      map.addSource("mesh-cells", {
        type: "geojson",
        data: { type: "FeatureCollection", features: cells_geom },
      });
      // Cost gradient for PASSABLE cells. Distinct from refusal —
      // a 2.5× cell is "expensive marsh", not "forbidden water".
      //   1.0×  =  no penalty   (translucent green)
      //   1.5×  =  mild slow     (yellow-green)
      //   2.5×+ =  expensive     (warm amber)
      // Refused cells get a different visual entirely (below).
      map.addLayer({
        id: "mesh-cells",
        type: "fill",
        source: "mesh-cells",
        paint: {
          "fill-color": [
            "interpolate", ["linear"], ["get", "cost"],
            1.0, "#86efac",   // sage
            1.5, "#fde047",   // lemon
            2.5, "#fb923c",   // warm orange
            4.0, "#92400e",   // deep amber
          ],
          "fill-opacity": 0.30,
        },
      });
      if (inspect.inspect.refused_polygons.length > 0) {
        map.addSource("mesh-refused", {
          type: "geojson",
          data: {
            type: "FeatureCollection",
            features: inspect.inspect.refused_polygons.map((ring) => ({
              type: "Feature" as const,
              properties: {},
              geometry: { type: "Polygon" as const, coordinates: [ring] },
            })),
          },
        });
        // Refused cells get a saturated red border so they stand
        // out from "merely expensive". Fill is lower opacity than
        // before so the underlying topo still reads through.
        map.addLayer({
          id: "mesh-refused",
          type: "fill",
          source: "mesh-refused",
          paint: { "fill-color": "#dc2626", "fill-opacity": 0.45 },
        });
        map.addLayer({
          id: "mesh-refused-outline",
          type: "line",
          source: "mesh-refused",
          paint: { "line-color": "#7f1d1d", "line-width": 1.0 },
        });
      }
    };
    if (map.isStyleLoaded()) apply();
    else map.once("load", apply);
  }, [inspect, showMesh]);

  // Render the path on the map (per-leg sources so we can colour
  // off-trail vs graph segments differently).
  useEffect(() => {
    const map = mapRef.current;
    if (!map) return;

    const ensure = () => {
      // Drop ALL previously-created leg layers/sources. Ids are
      // index-based (`path-<i>-<kind>`) because the unified solver emits
      // many legs of the SAME kind (off-trail/trail/off-trail/…) — keying
      // by kind alone collided on `addSource` (2nd throws), which aborted
      // rendering and left the route invisible / only the preview showing.
      for (const layer of map.getStyle().layers ?? []) {
        if (layer.id.startsWith("path-")) {
          map.removeLayer(layer.id);
          if (map.getSource(layer.id)) map.removeSource(layer.id);
        }
      }
      if (!path) return;
      const coords = path.path.geometry;
      path.path.legs.forEach((leg, i) => {
        const seg = coords.slice(leg.start_idx, leg.end_idx + 1);
        if (seg.length < 2) return;
        const id = `path-${i}-${leg.kind}`;
        map.addSource(id, {
          type: "geojson",
          data: {
            type: "Feature",
            geometry: { type: "LineString", coordinates: seg },
            properties: {},
          },
        });
        map.addLayer({
          id,
          type: "line",
          source: id,
          paint: {
            "line-color": LEG_COLOR[leg.kind] ?? CVD_VERMILLION,
            "line-width": 5,
            "line-opacity": 0.85,
          },
        });
      });
      // Invisible wide hit-line over the whole route so the curator can
      // grab it anywhere to insert a stop (drag-to-insert). Named with
      // the `path-` prefix so the cleanup loop above tears it down too.
      if (coords.length >= 2) {
        map.addSource("path-hit", {
          type: "geojson",
          data: { type: "Feature", geometry: { type: "LineString", coordinates: coords }, properties: {} },
        });
        map.addLayer({
          id: "path-hit",
          type: "line",
          source: "path-hit",
          paint: { "line-width": 18, "line-opacity": 0, "line-color": "#000" },
        });
      }
      // Fit-to-bounds.
      if (coords.length >= 2) {
        let minX = coords[0][0], minY = coords[0][1], maxX = coords[0][0], maxY = coords[0][1];
        for (const [x, y] of coords) {
          if (x < minX) minX = x;
          if (x > maxX) maxX = x;
          if (y < minY) minY = y;
          if (y > maxY) maxY = y;
        }
        map.fitBounds([[minX, minY], [maxX, maxY]], { padding: 80, maxZoom: 15, duration: 600 });
      }
    };

    if (map.isStyleLoaded()) ensure();
    else map.once("load", ensure);
  }, [path]);

  // Total-ascent for the result card. Sample a DEM elevation profile
  // along the route geometry and sum the positive deltas. Runs once
  // per solved route; failures (no DEM coverage) just leave gain null.
  useEffect(() => {
    const geom = path?.path.geometry;
    if (!geom || geom.length < 2) {
      setGainM(null);
      return;
    }
    let cancelled = false;
    setGainM(null);
    // Cap samples so a 50 km route doesn't post a megabyte of coords.
    const samples = Math.min(512, Math.max(32, geom.length));
    v1.post<{ elev_m: (number | null)[] }>("/elev/profile", {
      line: geom,
      samples,
    })
      .then((r) => {
        if (cancelled) return;
        let gain = 0;
        const z = r.elev_m;
        for (let i = 1; i < z.length; i++) {
          const a = z[i - 1];
          const b = z[i];
          if (a != null && b != null && b > a) gain += b - a;
        }
        setGainM(gain);
      })
      .catch(() => {
        if (!cancelled) setGainM(null);
      });
    return () => {
      cancelled = true;
    };
  }, [path]);

  // (Live preview is now driven imperatively by the rAF loop in
  // startLivePreview/stopLivePreview, wired into the stream callbacks —
  // no React-state render per snapshot, so it extends fluidly.)

  // ============================================================
  // Algorithm-replay animation.
  //
  // Flattens the recording's phase frames into a single
  // chronological event list. `replayIdx` is the count of events
  // already rendered. Four MapLibre overlays are recomputed on
  // index change:
  //   • replay-explored — every NodePopped event so far
  //   • replay-relaxed  — every EdgeRelaxed so far (faded by age)
  //   • replay-los      — every LineOfSightCast so far
  //   • replay-best     — the most recent BestPathSnapshot
  // The "snake trail" the curator wanted is the explored set
  // growing outward as `replayIdx` advances.
  // ============================================================
  const flatEvents = useMemo(() => {
    // Live mode: liveEvents is the source of truth (grows as SSE
    // events arrive). Record+replay mode: flatten the embedded
    // recording from the path response.
    if (recordOn && liveMode) return liveEvents;
    const rec = path?.path?.recording;
    if (!rec) return [];
    return rec.phases.flatMap((p) => p.events);
  }, [path, recordOn, liveMode, liveEvents]);

  // Reset the replay cursor when a new recording arrives. In live
  // mode we don't auto-play playback — the renderer just shows
  // whatever events have arrived so far, growing as more stream in.
  useEffect(() => {
    setReplayIdx(0);
    setReplayPlaying(recordOn && !liveMode && flatEvents.length > 0);
  }, [flatEvents.length, recordOn, liveMode]);

  // Animation loop. setInterval pace = 16ms × (1 / speed). Each
  // tick advances by enough events to feel smooth — the bigger
  // the recording, the more events per tick (otherwise a 60K-event
  // Marka solve would take 16 minutes at 1 event per frame).
  useEffect(() => {
    if (!replayPlaying || flatEvents.length === 0) return;
    const totalMs = 12000 / Math.max(replaySpeed, 0.05); // ~12 s baseline @1×
    const tickMs = 33; // ~30 fps
    const step = Math.max(1, Math.ceil(flatEvents.length / (totalMs / tickMs)));
    const id = setInterval(() => {
      setReplayIdx((i) => {
        const next = i + step;
        if (next >= flatEvents.length) {
          setReplayPlaying(false);
          return flatEvents.length;
        }
        return next;
      });
    }, tickMs);
    return () => clearInterval(id);
  }, [replayPlaying, flatEvents.length, replaySpeed]);

  // In live mode the "cursor" is always end-of-stream — every
  // event the SPA has received so far is on screen. This lets us
  // reuse the same overlay-rendering code path for both
  // record+replay (cursor controlled by play/scrub) and live
  // (cursor pinned to the tail).
  const effectiveCursor =
    recordOn && liveMode ? flatEvents.length : Math.min(replayIdx, flatEvents.length);

  // Render the four replay overlays at the current cursor.
  useEffect(() => {
    const map = mapRef.current;
    if (!map) return;
    const apply = () => {
      const ids = ["replay-explored", "replay-relaxed", "replay-los", "replay-best"];
      for (const id of ids) {
        if (map.getLayer(id)) map.removeLayer(id);
        if (map.getSource(id)) map.removeSource(id);
      }
      if (flatEvents.length === 0) return;
      const cursor = effectiveCursor;
      const explored: [number, number][] = [];
      const relaxedSegs: [number, number][][] = [];
      const losSegs: { coords: [number, number][]; blocked: boolean }[] = [];
      let bestPath: [number, number][] | null = null;
      for (let i = 0; i < cursor; i++) {
        const ev = flatEvents[i];
        if (ev.kind === "node_popped") {
          explored.push([ev.x, ev.y]);
        } else if (ev.kind === "edge_relaxed") {
          relaxedSegs.push([
            [ev.fx, ev.fy],
            [ev.tx, ev.ty],
          ]);
        } else if (ev.kind === "line_of_sight_cast") {
          losSegs.push({ coords: [[ev.fx, ev.fy], [ev.tx, ev.ty]], blocked: ev.blocked });
        } else if (ev.kind === "best_path_snapshot") {
          bestPath = ev.coords;
        }
      }
      if (explored.length > 0) {
        map.addSource("replay-explored", {
          type: "geojson",
          data: {
            type: "FeatureCollection",
            features: explored.map((c) => ({
              type: "Feature" as const,
              properties: {},
              geometry: { type: "Point" as const, coordinates: c },
            })),
          },
        });
        map.addLayer({
          id: "replay-explored",
          type: "circle",
          source: "replay-explored",
          paint: {
            "circle-color": "#1d4ed8",
            "circle-radius": 2,
            "circle-opacity": 0.35,
          },
        });
      }
      if (relaxedSegs.length > 0) {
        map.addSource("replay-relaxed", {
          type: "geojson",
          data: {
            type: "FeatureCollection",
            features: relaxedSegs.map((seg) => ({
              type: "Feature" as const,
              properties: {},
              geometry: { type: "LineString" as const, coordinates: seg },
            })),
          },
        });
        map.addLayer({
          id: "replay-relaxed",
          type: "line",
          source: "replay-relaxed",
          paint: { "line-color": "#fb923c", "line-width": 1, "line-opacity": 0.4 },
        });
      }
      if (losSegs.length > 0) {
        map.addSource("replay-los", {
          type: "geojson",
          data: {
            type: "FeatureCollection",
            features: losSegs.map((seg) => ({
              type: "Feature" as const,
              properties: { blocked: seg.blocked },
              geometry: { type: "LineString" as const, coordinates: seg.coords },
            })),
          },
        });
        map.addLayer({
          id: "replay-los",
          type: "line",
          source: "replay-los",
          paint: {
            "line-color": [
              "case",
              ["==", ["get", "blocked"], true],
              "#dc2626",
              "#facc15",
            ],
            "line-width": 0.6,
            "line-opacity": 0.5,
            "line-dasharray": [3, 2],
          },
        });
      }
      if (bestPath) {
        map.addSource("replay-best", {
          type: "geojson",
          data: {
            type: "Feature",
            geometry: { type: "LineString", coordinates: bestPath },
            properties: {},
          },
        });
        map.addLayer({
          id: "replay-best",
          type: "line",
          source: "replay-best",
          paint: { "line-color": "#0ea5e9", "line-width": 4, "line-opacity": 0.95 },
        });
      }
    };
    if (map.isStyleLoaded()) apply();
    else map.once("load", apply);
  }, [replayIdx, flatEvents, effectiveCursor]);

  const reset = () => {
    commitPoints([]);
    setPath(null);
    setErr(null);
  };
  const deletePoint = (i: number) =>
    commitPoints(pointsRef.current.filter((_, idx) => idx !== i));
  const reversePoints = () =>
    commitPoints([...pointsRef.current].reverse());
  const reorderPoints = (fromIdx: number, toIdx: number) => {
    if (fromIdx === toIdx) return;
    const next = [...pointsRef.current];
    const [moved] = next.splice(fromIdx, 1);
    next.splice(toIdx, 0, moved);
    commitPoints(next);
  };
  // Index of the row being dragged in the waypoint list (HTML5 DnD).
  const dragRowRef = useRef<number | null>(null);

  return (
    <div className="relative h-screen w-screen overflow-hidden bg-ink-100">
      {/* Full-bleed map; everything else floats over it. */}
      <div ref={containerRef} className="absolute inset-0" />

      {/* PRIMARY — planning surface as a center-bottom sheet (MD3), like
          a modern maps app: trip style, stops, and the result. */}
      <div className="absolute bottom-4 left-1/2 z-10 flex w-[min(620px,calc(100%-2rem))] max-h-[62vh] -translate-x-1/2 flex-col gap-4 overflow-y-auto rounded-[28px] border border-black/5 bg-white/95 p-5 shadow-2xl backdrop-blur-md">
          {/* Grabber. */}
          <div className="mx-auto -mb-1 h-1 w-10 shrink-0 rounded-full bg-ink-200" />

          {/* Travel mode — segmented control at the top, maps-style. */}
          <div className="flex rounded-full bg-ink-100 p-1 text-sm font-medium">
            {([
              ["foot", "Walk"],
              ["bicycle", "Bike"],
              ["ski", "Ski"],
            ] as [Profile, string][]).map(([id, label]) => (
              <button
                key={id}
                type="button"
                onClick={() => setProfile(id)}
                className={`flex-1 rounded-full px-3 py-1.5 transition-colors ${
                  profile === id
                    ? "bg-white text-ink-900 shadow-sm"
                    : "text-ink-500 hover:text-ink-800"
                }`}
              >
                {label}
              </button>
            ))}
          </div>

          {/* Trip style — horizontal chip row + one-line description. */}
          {presets.length > 0 ? (
            <div className="space-y-1.5">
              <div className="-mx-1 flex gap-2 overflow-x-auto px-1 pb-0.5">
                {presets.map((p) => {
                  const on = p.name === preset;
                  return (
                    <button
                      key={p.name}
                      type="button"
                      onClick={() => setPreset(p.name)}
                      aria-pressed={on}
                      className={`shrink-0 rounded-full px-3.5 py-1.5 text-sm font-medium transition-colors ${
                        on
                          ? "bg-[#0072B2] text-white"
                          : "bg-ink-100 text-ink-700 hover:bg-ink-200"
                      }`}
                    >
                      {p.label}
                    </button>
                  );
                })}
              </div>
              <p className="px-1 text-xs text-ink-500">
                {presets.find((p) => p.name === preset)?.description ?? ""}
              </p>
            </div>
          ) : null}

          {/* Status / result — the one thing the user looks at. */}
          <RouteStatus
            from={from}
            to={to}
            busy={busy}
            err={err}
            refusal={refusal}
            path={path}
            gainM={gainM}
            profile={profile}
          />

          {/* Stops — compact, collapsible, internally-scrolling so the sheet
              height stays fixed no matter how many stops are added. */}
          {points.length > 0 ? (
            <section className="space-y-1">
              <div className="flex items-center justify-between px-1">
                <button
                  type="button"
                  onClick={() => setStopsOpen((o) => !o)}
                  className="flex items-center gap-1 text-xs font-medium uppercase tracking-wide text-ink-400 hover:text-ink-700"
                >
                  <span className="text-[10px] leading-none">{stopsOpen ? "▾" : "▸"}</span>
                  Stops · {points.length}
                </button>
                <div className="flex items-center gap-0.5 text-ink-400">
                  <button type="button" title="Undo" onClick={undoPoints}
                    disabled={pointsPast.length === 0}
                    className="grid h-7 w-7 place-items-center rounded-full hover:bg-ink-100 disabled:opacity-30">↶</button>
                  <button type="button" title="Redo" onClick={redoPoints}
                    disabled={pointsFuture.length === 0}
                    className="grid h-7 w-7 place-items-center rounded-full hover:bg-ink-100 disabled:opacity-30">↷</button>
                  <button type="button" title="Reverse route" onClick={reversePoints}
                    disabled={points.length < 2}
                    className="grid h-7 w-7 place-items-center rounded-full hover:bg-ink-100 disabled:opacity-30">⇅</button>
                  <button type="button" title="Clear all" onClick={reset}
                    disabled={points.length === 0}
                    className="grid h-7 w-7 place-items-center rounded-full hover:bg-ink-100 disabled:opacity-30">✕</button>
                </div>
              </div>
              {stopsOpen ? (
                <ol className="max-h-[26vh] overflow-y-auto pr-1">
                  {points.map((pt, i) => {
                    const isStart = i === 0;
                    const isEnd = i === points.length - 1 && points.length >= 2;
                    const bg = isStart ? CVD_BLUE : isEnd ? CVD_VERMILLION : "#6b7280";
                    const legToNext = path?.path.waypoint_legs?.[i];
                    const refusedHere =
                      (refusal?.which === "from" && isStart) ||
                      (refusal?.which === "to" && isEnd);
                    const name = isStart ? "Start" : isEnd ? "Destination" : `Stop ${i}`;
                    return (
                      <li
                        key={i}
                        draggable
                        onDragStart={() => { dragRowRef.current = i; }}
                        onDragOver={(e) => e.preventDefault()}
                        onDrop={() => {
                          if (dragRowRef.current != null) reorderPoints(dragRowRef.current, i);
                          dragRowRef.current = null;
                        }}
                        className="group flex items-center gap-2.5 rounded-lg px-1.5 py-1 hover:bg-ink-50"
                      >
                        <span
                          className="h-2.5 w-2.5 shrink-0 rounded-full ring-2 ring-white"
                          style={{ background: bg, boxShadow: refusedHere ? "0 0 0 2px #dc2626" : undefined }}
                        />
                        <span className="min-w-0 flex-1 truncate text-sm text-ink-800">
                          <span className="font-medium">{name}</span>
                          <span className="text-xs tabular-nums text-ink-400">
                            {legToNext
                              ? ` · ${fmtDist(legToNext.length_m)}`
                              : ` · ${pt[1].toFixed(3)}, ${pt[0].toFixed(3)}`}
                          </span>
                        </span>
                        <span title="Drag to reorder" className="cursor-grab select-none px-1 text-ink-300 opacity-0 group-hover:opacity-100">⠿</span>
                        <button type="button" title="Remove" onClick={() => deletePoint(i)}
                          className="px-1 text-ink-300 hover:text-rose-600 opacity-0 group-hover:opacity-100">✕</button>
                      </li>
                    );
                  })}
                </ol>
              ) : null}
            </section>
          ) : null}

        </div>{/* /LEFT card */}

        {/* RIGHT — debug / calibration pane, collapsible so it stays out
            of the way. Collapsed = a small icon button; open = full card. */}
        {!debugOpen ? (
          <button
            type="button"
            onClick={() => setDebugOpen(true)}
            title="Debug & layers"
            className="absolute top-4 right-4 z-10 grid h-11 w-11 place-items-center rounded-full border border-black/5 bg-white/90 text-lg text-ink-600 shadow-2xl backdrop-blur-md hover:bg-white"
          >
            ⚙
          </button>
        ) : (
        <div className="absolute top-4 right-4 z-10 flex max-h-[calc(100vh-2rem)] w-[340px] flex-col gap-4 overflow-y-auto rounded-[28px] border border-black/5 bg-white/90 p-5 shadow-2xl backdrop-blur-md">
          <div className="flex items-center justify-between">
            <span className="text-sm font-semibold tracking-tight">Debug &amp; layers</span>
            <button
              type="button"
              onClick={() => setDebugOpen(false)}
              title="Close"
              className="grid h-7 w-7 place-items-center rounded-full text-ink-500 hover:bg-ink-100"
            >
              ✕
            </button>
          </div>
          <section className="space-y-2">
            <div className="text-xs uppercase tracking-wide text-ink-500">
              Show on map
            </div>
            <OverlayChip
              active={showWater}
              setActive={setShowWater}
              label="water/glacier"
              color="#1e3a8a"
              count={waterCells?.returned}
              capped={waterCells?.bbox_clipped}
            />
            <OverlayChip
              active={showWetland}
              setActive={setShowWetland}
              label="wetland (myr)"
              color="#7c3aed"
              count={wetlandCells?.returned}
              capped={wetlandCells?.bbox_clipped}
            />
            <OverlayChip
              active={showForest}
              setActive={setShowForest}
              label="forest (skog)"
              color="#15803d"
              count={forestCells?.returned}
              capped={forestCells?.bbox_clipped}
            />
            <OverlayChip
              active={showEdgesSti}
              setActive={setShowEdgesSti}
              label="trails (sti)"
              color="#dc2626"
              count={stiEdges?.returned}
              capped={stiEdges?.capped}
            />
            <OverlayChip
              active={showEdgesVei}
              setActive={setShowEdgesVei}
              label="roads (vei)"
              color="#374151"
              count={veiEdges?.returned}
              capped={veiEdges?.capped}
            />
            <OverlayChip
              active={showEdgesSki}
              setActive={setShowEdgesSki}
              label="ski tracks"
              color="#0ea5e9"
              count={skiEdges?.returned}
              capped={skiEdges?.capped}
            />
            <OverlayChip
              active={showAnchors}
              setActive={setShowAnchors}
              label="anchors"
              color="#a16207"
              count={anchorPts?.returned}
              capped={anchorPts?.capped}
            />
          </section>

          <section className="space-y-2">
            <div className="text-xs uppercase tracking-wide text-ink-500">Basemap</div>
            <div className="grid grid-cols-2 gap-1.5">
              {(Object.keys(BASEMAPS) as BasemapId[]).map((id) => (
                <button
                  key={id}
                  type="button"
                  onClick={() => setBasemap(id)}
                  className={`text-xs px-2 py-1.5 rounded border ${
                    basemap === id
                      ? "border-ink-900 bg-ink-900 text-ink-50"
                      : "border-ink-200 hover:bg-ink-100"
                  }`}
                >
                  {BASEMAPS[id].label}
                </button>
              ))}
            </div>
          </section>

          {/* ============================================================
              Advanced (testing) — every developer / calibration control
              lives here, folded away by default. Toggle open for solver
              knobs, layer weights, cost calibration, the mesh inspector,
              algorithm replay, and coverage overlays. None of this is
              needed for normal route planning.
              ============================================================ */}
          <section className="border-t border-ink-200 pt-4">
            <button
              type="button"
              onClick={() => setAdvancedOpen((o) => !o)}
              className="w-full flex items-center gap-2 text-xs uppercase tracking-wide text-ink-400 hover:text-ink-700"
              data-testid="advanced-toggle"
            >
              <span>{advancedOpen ? "▾" : "▸"}</span>
              <span>Advanced (testing)</span>
            </button>
          </section>

          {advancedOpen ? (
          <div className="space-y-5" data-testid="advanced-panel">
          <section className="space-y-2">
            <div className="text-xs uppercase tracking-wide text-ink-500">Debug</div>
            <label className="text-sm flex items-center gap-2">
              <input
                type="checkbox"
                checked={showMesh}
                onChange={(e) => setShowMesh(e.target.checked)}
              />
              <span>
                Show pathfind mesh{" "}
                {inspect ? (
                  <span className="text-xs text-ink-500">
                    ({inspect.inspect.cells.length} cells,{" "}
                    {inspect.inspect.refused_polygons.length} refused,{" "}
                    {inspect.took_us} µs)
                  </span>
                ) : null}
              </span>
            </label>
            <label className="text-sm flex items-center gap-2">
              <input
                type="checkbox"
                checked={inspectMode}
                onChange={(e) => {
                  setInspectMode(e.target.checked);
                  if (!e.target.checked) setCellInfo(null);
                }}
              />
              <span>
                <strong>Click cells to inspect</strong>{" "}
                <span className="text-xs text-ink-500">
                  (overrides marker placement)
                </span>
              </span>
            </label>
            <label className="text-sm flex items-center gap-2">
              <input
                type="checkbox"
                checked={forceOffTrail}
                onChange={(e) => setForceOffTrail(e.target.checked)}
              />
              <span>
                <strong>Force off-trail</strong>{" "}
                <span className="text-xs text-ink-500">
                  (skip graph — use when trail topology is sparse)
                </span>
              </span>
            </label>
            <div className="text-xs text-ink-500 mt-1">
              Mesh colours:{" "}
              <span className="inline-block w-3 h-3 rounded mr-1 align-middle" style={{ backgroundColor: "#86efac", opacity: 0.6 }} /> open ·{" "}
              <span className="inline-block w-3 h-3 rounded mr-1 align-middle" style={{ backgroundColor: "#fde047", opacity: 0.6 }} /> mild ·{" "}
              <span className="inline-block w-3 h-3 rounded mr-1 align-middle" style={{ backgroundColor: "#fb923c", opacity: 0.6 }} /> expensive ·{" "}
              <span className="inline-block w-3 h-3 rounded mr-1 align-middle" style={{ backgroundColor: "#dc2626", opacity: 0.6 }} /> refused
            </div>

            {/* ---- Algorithm replay --------------------------- */}
            <label className="text-sm flex items-center gap-2 mt-3">
              <input
                type="checkbox"
                checked={recordOn}
                onChange={(e) => setRecordOn(e.target.checked)}
                data-testid="record-toggle"
              />
              <span>
                <strong>Record algorithm replay</strong>{" "}
                <span className="text-xs text-ink-500">
                  (animates the solver's exploration — ~10% slower, larger payload)
                </span>
              </span>
            </label>
            {recordOn ? (
              <label className="text-sm flex items-center gap-2 ml-6">
                <input
                  type="checkbox"
                  checked={liveMode}
                  onChange={(e) => setLiveMode(e.target.checked)}
                  data-testid="live-mode-toggle"
                />
                <span>
                  <strong>Live mode</strong>{" "}
                  <span className="text-xs text-ink-500">
                    (stream solver events as they happen — best for long off-trail solves)
                  </span>
                </span>
              </label>
            ) : null}
            {recordOn && liveMode && (liveEvents.length > 0 || liveDone) ? (
              <div className="text-xs text-ink-600 mt-1 space-y-1" data-testid="live-panel">
                <div>
                  <span className={liveDone ? "text-ink-500" : "text-emerald-600 font-medium"}>
                    {liveDone ? "● live (done)" : "● live (streaming)"}
                  </span>
                  {" · "}events received: <code>{liveEvents.length}</code>
                  {liveEvents.length >= 50000 ? (
                    <span className="text-amber-700"> (capped at 50K)</span>
                  ) : null}
                </div>
                <div className="text-xs text-ink-500">
                  Legend:{" "}
                  <span className="inline-block w-3 h-3 rounded-full mr-1 align-middle" style={{ backgroundColor: "#1d4ed8", opacity: 0.6 }} /> explored ·{" "}
                  <span className="inline-block w-3 h-1 mr-1 align-middle" style={{ backgroundColor: "#fb923c" }} /> relaxed ·{" "}
                  <span className="inline-block w-3 h-1 mr-1 align-middle" style={{ backgroundColor: "#facc15" }} /> LoS hit ·{" "}
                  <span className="inline-block w-3 h-1 mr-1 align-middle" style={{ backgroundColor: "#dc2626" }} /> LoS blocked ·{" "}
                  <span className="inline-block w-3 h-1 mr-1 align-middle" style={{ backgroundColor: "#0ea5e9" }} /> best path
                </div>
              </div>
            ) : recordOn && path?.path.recording ? (
              <div className="text-xs text-ink-600 mt-1 space-y-1" data-testid="replay-panel">
                <div>
                  Events: <code>{path.path.recording.events_retained}</code>
                  {path.path.recording.decimated ? (
                    <span className="text-amber-700">
                      {" "}(decimated from {path.path.recording.events_observed})
                    </span>
                  ) : null}
                  {" · "}cursor: <code>{replayIdx}</code> / {flatEvents.length}
                </div>
                <div className="flex items-center gap-2">
                  <button
                    type="button"
                    className="px-2 py-0.5 text-xs rounded border border-ink-300 hover:bg-ink-100"
                    onClick={() => {
                      if (replayIdx >= flatEvents.length) setReplayIdx(0);
                      setReplayPlaying((p) => !p);
                    }}
                    data-testid="replay-play"
                  >
                    {replayPlaying ? "⏸ pause" : replayIdx >= flatEvents.length ? "⟲ replay" : "▶ play"}
                  </button>
                  <button
                    type="button"
                    className="px-2 py-0.5 text-xs rounded border border-ink-300 hover:bg-ink-100"
                    onClick={() => {
                      setReplayPlaying(false);
                      setReplayIdx(0);
                    }}
                    data-testid="replay-reset"
                  >
                    ⏮ reset
                  </button>
                  <label className="text-xs">
                    speed{" "}
                    <select
                      value={replaySpeed}
                      onChange={(e) => setReplaySpeed(parseFloat(e.target.value))}
                      className="border border-ink-300 rounded text-xs"
                    >
                      <option value="0.25">0.25×</option>
                      <option value="1">1×</option>
                      <option value="4">4×</option>
                      <option value="16">16×</option>
                    </select>
                  </label>
                </div>
                <input
                  type="range"
                  min={0}
                  max={flatEvents.length}
                  value={replayIdx}
                  onChange={(e) => {
                    setReplayPlaying(false);
                    setReplayIdx(parseInt(e.target.value, 10));
                  }}
                  className="w-full"
                  data-testid="replay-scrub"
                />
                <div className="text-xs text-ink-500">
                  Legend:{" "}
                  <span className="inline-block w-3 h-3 rounded-full mr-1 align-middle" style={{ backgroundColor: "#1d4ed8", opacity: 0.6 }} /> explored ·{" "}
                  <span className="inline-block w-3 h-1 mr-1 align-middle" style={{ backgroundColor: "#fb923c" }} /> relaxed ·{" "}
                  <span className="inline-block w-3 h-1 mr-1 align-middle" style={{ backgroundColor: "#facc15" }} /> LoS hit ·{" "}
                  <span className="inline-block w-3 h-1 mr-1 align-middle" style={{ backgroundColor: "#dc2626" }} /> LoS blocked ·{" "}
                  <span className="inline-block w-3 h-1 mr-1 align-middle" style={{ backgroundColor: "#0ea5e9" }} /> best path
                </div>
              </div>
            ) : recordOn ? (
              <div className="text-xs text-ink-500 mt-1">
                Place both markers to record a route.
              </div>
            ) : null}
            {inspect && inspect.inspect.refused_by.length > 0 ? (
              <div className="text-xs text-amber-700">
                Refused cells from:{" "}
                {inspect.inspect.refused_by.map((r) => (
                  <code
                    key={r}
                    className="bg-amber-100 px-1 rounded mr-1"
                  >
                    {r}
                  </code>
                ))}
              </div>
            ) : null}
            {cellInfo ? (
              <div className="bg-white border border-ink-300 rounded p-3 space-y-2 text-xs">
                <div className="flex items-center justify-between">
                  <span className="font-medium">
                    Cell @ {cellInfo.point.lon.toFixed(5)},{" "}
                    {cellInfo.point.lat.toFixed(5)}
                  </span>
                  <button
                    type="button"
                    onClick={() => setCellInfo(null)}
                    className="text-ink-400 hover:text-ink-900"
                  >
                    ×
                  </button>
                </div>
                <div>
                  <span className="text-ink-500">UTM33N: </span>
                  <code>
                    {cellInfo.point.x_25833.toFixed(0)},{" "}
                    {cellInfo.point.y_25833.toFixed(0)}
                  </code>
                </div>
                <div>
                  <span className="text-ink-500">Composed: </span>
                  <code
                    className={`px-1 rounded ${
                      cellInfo.point.refused_by
                        ? "bg-red-100 text-red-800"
                        : cellInfo.point.composed_multiplier > 2
                          ? "bg-amber-100"
                          : ""
                    }`}
                  >
                    {cellInfo.point.refused_by
                      ? `refused (${cellInfo.point.refused_by})`
                      : `${cellInfo.point.composed_multiplier.toFixed(2)}×`}
                  </code>{" "}
                  <span className="text-ink-500">({cellInfo.took_us} µs)</span>
                </div>
                <table className="w-full text-xs">
                  <thead>
                    <tr className="text-ink-500 text-left">
                      <th className="py-0.5">layer</th>
                      <th className="py-0.5">covers</th>
                      <th className="py-0.5 text-right">mult.</th>
                      <th className="py-0.5">refused</th>
                    </tr>
                  </thead>
                  <tbody>
                    {cellInfo.point.layers.map((l) => (
                      <tr
                        key={l.name}
                        className={
                          l.refused
                            ? "bg-red-50"
                            : l.multiplier > 1.5
                              ? "bg-amber-50"
                              : ""
                        }
                      >
                        <td className="py-0.5 font-mono">{l.name}</td>
                        <td className="py-0.5">{l.covers ? "✓" : "—"}</td>
                        <td className="py-0.5 text-right font-mono">
                          {l.multiplier.toFixed(2)}×
                        </td>
                        <td className="py-0.5">{l.refused ?? ""}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            ) : inspectMode ? (
              <div className="text-xs text-ink-500 bg-ink-50 border border-ink-200 rounded p-2">
                Click anywhere on the map to inspect that cell. Toggle off
                to resume placing markers.
              </div>
            ) : null}
          </section>

          <section className="space-y-2">
            <div className="text-xs uppercase tracking-wide text-ink-500">Coverage</div>
            <label className="text-sm flex items-center gap-2">
              <input
                type="checkbox"
                checked={showCoverage}
                onChange={(e) => setShowCoverage(e.target.checked)}
              />
              <span>Show DEM / mask / graph extents</span>
            </label>
            <label className="text-sm flex items-center gap-2">
              <input
                type="checkbox"
                checked={showDensity}
                onChange={(e) => setShowDensity(e.target.checked)}
              />
              <span>
                Show graph density (
                {graphDensity?.returned_count ?? 0} of{" "}
                {graphDensity?.source_count ?? 0} nodes sampled)
              </span>
            </label>
            <div className="text-xs text-ink-500 space-y-0.5 pt-1">
              {graphStats ? (
                <div>
                  <span className="inline-block w-2 h-2 rounded-sm mr-1.5 align-middle" style={{ backgroundColor: "#f59e0b" }} />
                  graph: {graphStats.meta.node_count.toLocaleString()} nodes,{" "}
                  {graphStats.meta.edge_count.toLocaleString()} edges
                </div>
              ) : null}
              {maskCoverage ? (
                <div>
                  <span className="inline-block w-2 h-2 rounded-sm mr-1.5 align-middle" style={{ backgroundColor: "#22c55e" }} />
                  mask: {maskCoverage.cells_water.toLocaleString()} water +{" "}
                  {maskCoverage.cells_glacier.toLocaleString()} glacier cells
                </div>
              ) : null}
              {demCoverage ? (
                <div>
                  <span className="inline-block w-2 h-2 rounded-sm mr-1.5 align-middle" style={{ backgroundColor: "#0ea5e9" }} />
                  dem: {demCoverage.cells_x}×{demCoverage.cells_y} @{" "}
                  {demCoverage.resolution_m}m
                </div>
              ) : (
                <div className="text-amber-700">⚠ DEM artifact not loaded</div>
              )}
            </div>
          </section>

          <section className="space-y-3">
            <div className="text-xs uppercase tracking-wide text-ink-500">Solver knobs</div>
            <RangeRow
              label="snap_radius_m"
              value={snapRadius}
              setValue={setSnapRadius}
              min={50}
              max={1000}
              step={50}
              hint="Below this distance, an endpoint snaps directly onto the graph."
            />
            <RangeRow
              label="bridge_radius_m"
              value={bridgeRadius}
              setValue={setBridgeRadius}
              min={500}
              max={10000}
              step={500}
              hint="Within this, a hybrid path bridges off-trail prefix → graph → off-trail suffix."
            />
            <RangeRow
              label="mesh_cell_m"
              value={meshCell}
              setValue={setMeshCell}
              min={25}
              max={250}
              step={25}
              hint="Off-trail mesh resolution. Smaller = smoother + slower."
            />
            <RangeRow
              label="mesh_pad_m"
              value={meshPad}
              setValue={setMeshPad}
              min={0}
              max={10000}
              step={500}
              hint="Search corridor width around the [from, to] line. 0 = auto (≥ 30 % of route length). Bigger lets the solver detour around lakes."
              format={(v) => (v === 0 ? "auto" : `${v}`)}
            />
            <RangeRow
              label="refusal_snap_m"
              value={refusalSnap}
              setValue={setRefusalSnap}
              min={0}
              max={500}
              step={25}
              hint="If a click lands in a tiny water sliver, snap outward this far before refusing."
            />
          </section>

          <section className="space-y-3">
            <div className="text-xs uppercase tracking-wide text-ink-500">
              Layer weights ({layerNames.length})
            </div>
            {layerNames.length === 0 ? (
              <p className="text-xs text-ink-500">
                No layers registered. Make sure DEM + mask artifacts loaded at boot.
              </p>
            ) : (
              layerNames.map((name) => (
                <RangeRow
                  key={name}
                  label={name}
                  value={layerWeights[name] ?? 1.0}
                  setValue={(v) =>
                    setLayerWeights((w) => ({ ...w, [name]: v }))
                  }
                  min={0}
                  max={2}
                  step={0.1}
                  format={(v) => `${v.toFixed(1)}×`}
                />
              ))
            )}
          </section>

          <section className="space-y-2" data-testid="cost-calibration-section">
            <button
              type="button"
              className="text-xs uppercase tracking-wide text-ink-500 hover:text-ink-900 flex items-center gap-1"
              onClick={() => setCostPanelOpen((o) => !o)}
              data-testid="cost-panel-toggle"
            >
              <span>{costPanelOpen ? "▾" : "▸"}</span>
              <span>Cost calibration</span>
              {nonNullPatch(costPatch) ? (
                <span className="ml-1 text-emerald-700 normal-case">
                  · overriding {Object.values(costPatch).filter((v) => v !== null).length}
                </span>
              ) : null}
            </button>
            {costPanelOpen ? (
              <div className="space-y-2" data-testid="cost-panel">
                <p className="text-xs text-ink-500 leading-relaxed">
                  Per-request patch over the boot{" "}
                  <code className="bg-ink-100 px-1 rounded">cost-config.toml</code>.
                  Sliders start at the live boot value; toggle a row on to
                  override it for the next solve. Affects this session only —
                  the server config is untouched.
                </p>
                {[
                  { key: "off_trail_base_foot" as const, label: "off_trail.foot", min: 1.0, max: 2.5, step: 0.05, format: (v: number) => v.toFixed(2) },
                  { key: "off_trail_base_bicycle" as const, label: "off_trail.bike", min: 1.0, max: 4.0, step: 0.05, format: (v: number) => v.toFixed(2) },
                  { key: "off_trail_base_ski" as const, label: "off_trail.ski", min: 0.5, max: 2.0, step: 0.05, format: (v: number) => v.toFixed(2) },
                  { key: "trail_proximity_bonus_at_zero" as const, label: "trail_prox.bonus", min: 0.5, max: 1.0, step: 0.01, format: (v: number) => v.toFixed(2) },
                  { key: "trail_proximity_influence_radius_m" as const, label: "trail_prox.r_m", min: 10, max: 200, step: 5, format: (v: number) => `${v.toFixed(0)} m` },
                  { key: "slope_cell_quadratic_scale_deg" as const, label: "slope_cell.k_deg", min: 5, max: 60, step: 1, format: (v: number) => `${v.toFixed(0)}°` },
                  { key: "slope_cell_refuse_above_deg" as const, label: "slope_cell.refuse", min: 30, max: 70, step: 1, format: (v: number) => `${v.toFixed(0)}°` },
                  { key: "slope_graph_quadratic_scale_deg" as const, label: "slope_graph.k_deg", min: 5, max: 60, step: 1, format: (v: number) => `${v.toFixed(0)}°` },
                  { key: "slope_graph_refuse_above_deg" as const, label: "slope_graph.refuse", min: 30, max: 70, step: 1, format: (v: number) => `${v.toFixed(0)}°` },
                  { key: "total_gain_amplifier" as const, label: "total_gain.amp", min: 0.0, max: 2.0, step: 0.05, format: (v: number) => v.toFixed(2) },
                ].map((row) => {
                  const overridden = costPatch[row.key] !== null;
                  const boot = costConfigBoot?.[row.key];
                  const display = overridden ? costPatch[row.key]! : (boot ?? row.min);
                  return (
                    <div key={row.key} className="flex items-center gap-2" data-testid={`cost-row-${row.key}`}>
                      <input
                        type="checkbox"
                        checked={overridden}
                        onChange={(e) =>
                          setCostPatch((p) => ({
                            ...p,
                            [row.key]: e.target.checked ? (boot ?? (row.min + row.max) / 2) : null,
                          }))
                        }
                        title="Override this knob for the next solve"
                        data-testid={`cost-toggle-${row.key}`}
                      />
                      <code className="text-xs bg-ink-100 px-1 py-0.5 rounded w-32 inline-block">
                        {row.label}
                      </code>
                      <input
                        type="range"
                        min={row.min}
                        max={row.max}
                        step={row.step}
                        value={display}
                        disabled={!overridden}
                        onChange={(e) =>
                          setCostPatch((p) => ({
                            ...p,
                            [row.key]: parseFloat(e.target.value),
                          }))
                        }
                        className="flex-1"
                        data-testid={`cost-range-${row.key}`}
                      />
                      <span className="text-xs tabular-nums w-12 text-right">
                        {row.format(display)}
                      </span>
                      {boot !== undefined && !overridden ? (
                        <span className="text-[10px] text-ink-400" title="Boot config value">
                          (boot)
                        </span>
                      ) : null}
                    </div>
                  );
                })}
                <button
                  type="button"
                  className="text-xs underline text-ink-600 hover:text-ink-900"
                  onClick={() => setCostPatch(EMPTY_PATCH)}
                  disabled={!nonNullPatch(costPatch)}
                  data-testid="cost-reset"
                >
                  Reset all to boot config
                </button>
              </div>
            ) : null}
          </section>

          {path ? <ResultPanel resp={path} /> : null}

          {demCoverage ? (
            <section className="space-y-1 text-xs text-ink-500 border-t border-ink-200 pt-3">
              <div className="font-medium text-ink-700">DEM coverage</div>
              <div>
                {demCoverage.cells_x} × {demCoverage.cells_y} cells @{" "}
                {demCoverage.resolution_m} m ({demCoverage.tiles_present} tiles)
              </div>
            </section>
          ) : null}
          </div>
          ) : null}
        </div>
        )}{/* /RIGHT card */}
    </div>
  );
}

/// Strip null/empty entries from a CostConfigPatch and return
/// `null` if every override is unset, so the request body skips
/// `cost_config_override` entirely (server falls back to the boot
/// config). Returning a sparse object lets `with_patch` keep its
/// inheritance semantics — unset rows inherit, set rows override.
function nonNullPatch(
  patch: Record<string, number | null>,
): Record<string, number> | null {
  const out: Record<string, number> = {};
  let any = false;
  for (const [k, v] of Object.entries(patch)) {
    if (v !== null && Number.isFinite(v)) {
      out[k] = v;
      any = true;
    }
  }
  return any ? out : null;
}

/// Consume an SSE stream from `/v1/pathfind/stream`. The endpoint
/// is POST-bodied (EventSource only supports GET) so we drive it
/// via fetch + a ReadableStream reader. Each `event: solver` frame
/// becomes one `onSolver` callback; the terminal `event: done` or
/// `event: error` ends the stream.
///
/// Callers pass an AbortSignal so a marker click that supersedes
/// the previous query can cancel the in-flight stream cleanly.
async function streamPathfind(
  points: [number, number][],
  preset: string | null,
  prefs: Record<string, unknown>,
  signal: AbortSignal,
  cb: {
    onSolver: (ev: import("../api/v1").SolverEvent) => void;
    onDone: (resp: import("../api/v1").PathfindResp["path"]) => void;
    onError: (msg: string) => void;
  },
) {
  try {
    const resp = await fetch("/v1/pathfind/stream", {
      method: "POST",
      credentials: "include",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(preset ? { points, preset, prefs } : { points, prefs }),
      signal,
    });
    if (!resp.ok || !resp.body) {
      cb.onError(`HTTP ${resp.status}`);
      return;
    }
    const reader = resp.body
      .pipeThrough(new TextDecoderStream())
      .getReader();
    // SSE frames are separated by blank lines (\n\n). We accumulate
    // bytes between blank lines, then parse the `event:` + `data:`
    // pair. The fetch ReadableStream gives chunks; we split on
    // blank lines manually.
    let buf = "";
    while (true) {
      const { value, done } = await reader.read();
      if (done) break;
      buf += value;
      let idx: number;
      while ((idx = buf.indexOf("\n\n")) !== -1) {
        const frame = buf.slice(0, idx);
        buf = buf.slice(idx + 2);
        let eventName = "message";
        let dataLine = "";
        for (const line of frame.split("\n")) {
          if (line.startsWith("event:")) eventName = line.slice(6).trim();
          else if (line.startsWith("data:")) dataLine += line.slice(5).trim();
        }
        if (!dataLine) continue;
        if (eventName === "solver") {
          try {
            cb.onSolver(JSON.parse(dataLine));
          } catch {
            /* malformed event — ignore one frame */
          }
        } else if (eventName === "done") {
          try {
            cb.onDone(JSON.parse(dataLine));
          } catch (e) {
            cb.onError(`parse done: ${(e as Error).message}`);
          }
          return;
        } else if (eventName === "error") {
          try {
            const parsed = JSON.parse(dataLine);
            cb.onError(parsed.message ?? dataLine);
          } catch {
            cb.onError(dataLine);
          }
          return;
        }
      }
    }
  } catch (e) {
    if ((e as Error).name === "AbortError") return;
    cb.onError((e as Error).message);
  }
}

function OverlayChip({
  active,
  setActive,
  label,
  color,
  count,
  capped,
  testid,
}: {
  active: boolean;
  setActive: (v: boolean) => void;
  label: string;
  color: string;
  count: number | undefined;
  capped: boolean | undefined;
  testid?: string;
}) {
  // The test-id slug defaults to the kebab-case form of the label so
  // a Playwright `getByTestId('overlay-trails-sti')` survives copy
  // edits. Explicit `testid` overrides for callers that want a
  // stable slug independent of the visible text.
  const slug =
    testid ??
    "overlay-" +
      label
        .toLowerCase()
        .replace(/[()]/g, "")
        .replace(/[^a-z0-9]+/g, "-")
        .replace(/(^-|-$)/g, "");
  return (
    <label
      className={`text-sm flex items-center gap-2 px-2 py-1 rounded border ${
        active
          ? "border-ink-300 bg-ink-50"
          : "border-transparent hover:bg-ink-50"
      }`}
      data-testid={slug}
    >
      <input
        type="checkbox"
        checked={active}
        onChange={(e) => setActive(e.target.checked)}
        data-testid={`${slug}-checkbox`}
      />
      <span
        className="inline-block w-3 h-3 rounded"
        style={{ backgroundColor: color }}
      />
      <span className="flex-1">{label}</span>
      {active ? (
        <span className="text-xs text-ink-500">
          {count !== undefined ? `${count}${capped ? "+" : ""}` : "…"}
        </span>
      ) : null}
    </label>
  );
}

/**
 * The single thing the route-planner user reads: what's happening and,
 * once solved, the headline hiker stats. Collapses the old scattered
 * busy / error / refusal / result blocks into one calm card.
 */
function RouteStatus({
  from,
  to,
  busy,
  err,
  refusal,
  path,
  gainM,
  profile,
}: {
  from: Marker | null;
  to: Marker | null;
  busy: boolean;
  err: string | null;
  refusal: { which: "from" | "to"; layer: string } | null;
  path: PathfindResp | null;
  gainM: number | null;
  profile: Profile;
}) {
  // Refusal — a click landed in water/glacier/cliff. Friendly, no jargon.
  if (err && refusal) {
    return (
      <div className="rounded-xl bg-amber-50 border border-amber-200 p-4 text-sm text-amber-900">
        <div className="font-medium">
          Your {refusal.which === "from" ? "start" : "destination"} is on
          impassable ground
        </div>
        <div className="text-xs mt-1 text-amber-800">
          {refusal.layer === "mask_refusal"
            ? "That spot is a lake or glacier. Pick a point on land."
            : "Pick a point that isn't a cliff or open water."}
        </div>
      </div>
    );
  }
  if (err) {
    return (
      <div className="rounded-xl bg-rose-50 border border-rose-200 p-4 text-sm text-rose-700">
        Couldn't find a route. {err}
      </div>
    );
  }
  if (busy) {
    return (
      <div className="rounded-xl bg-ink-50 border border-ink-200 p-4 flex items-center gap-3 text-sm text-ink-600">
        <span className="inline-block w-4 h-4 rounded-full border-2 border-ink-300 border-t-ink-700 animate-spin" />
        Finding the best route…
      </div>
    );
  }
  if (path) {
    return <ResultCard resp={path} gainM={gainM} profile={profile} />;
  }
  // Idle guidance based on which markers are placed.
  const msg = !from
    ? "Tap the map to drop your start point."
    : !to
      ? "Now tap your destination."
      : "Drag the markers or tap again to re-plan.";
  return (
    <div className="rounded-xl bg-ink-50 border border-dashed border-ink-300 p-4 text-sm text-ink-500">
      {msg}
    </div>
  );
}

/** Headline hiker stats for a solved route. */
function ResultCard({
  resp,
  gainM,
  profile,
}: {
  resp: PathfindResp;
  gainM: number | null;
  profile: Profile;
}) {
  const p = resp.path;
  const km = p.length_m / 1000;
  const mins = naismithMinutes(p.length_m, gainM, profile);
  // Surface mix (metres by surface) → a thin MD3 stacked bar + legend.
  const fkb = p.fkb_breakdown ?? {};
  const surfaces: [string, string, string][] = [
    ["sti", "Trail", CVD_BLUE],
    ["vei", "Road", "#374151"],
    ["skiloype", "Ski track", "#0ea5e9"],
    ["off_trail", "Off-trail", CVD_VERMILLION],
    ["unknown", "Other", "#9ca3af"],
  ];
  const segs = surfaces
    .map(([k, label, color]) => ({ label, color, m: fkb[k] ?? 0 }))
    .filter((s) => s.m > 0);
  const segTotal = segs.reduce((a, s) => a + s.m, 0) || 1;
  return (
    <div className="rounded-2xl bg-ink-50 p-4 space-y-3" data-testid="route-result">
      <div className="flex items-baseline gap-2">
        <span className="text-3xl font-semibold tabular-nums tracking-tight">
          {km < 10 ? km.toFixed(1) : km.toFixed(0)} km
        </span>
        <span className="text-ink-300">·</span>
        <span className="text-xl font-medium tabular-nums text-ink-600">
          {formatDuration(mins)}
        </span>
        {gainM != null ? (
          <span className="ml-auto text-sm text-ink-500 tabular-nums">
            ↑ {gainM.toFixed(0)} m
          </span>
        ) : null}
      </div>
      {segs.length > 0 ? (
        <div className="space-y-1.5">
          <div className="flex h-2 overflow-hidden rounded-full">
            {segs.map((s) => (
              <div
                key={s.label}
                style={{ width: `${(s.m / segTotal) * 100}%`, background: s.color }}
                title={`${s.label}: ${fmtDist(s.m)}`}
              />
            ))}
          </div>
          <div className="flex flex-wrap gap-x-3 gap-y-0.5 text-xs text-ink-500">
            {segs.map((s) => (
              <span key={s.label} className="inline-flex items-center gap-1">
                <span
                  className="inline-block h-2 w-2 rounded-full"
                  style={{ background: s.color }}
                />
                {s.label} {Math.round((s.m / segTotal) * 100)}%
              </span>
            ))}
          </div>
        </div>
      ) : null}
    </div>
  );
}

/**
 * Naismith-style walking-time estimate from distance + ascent. Honest
 * and solver-independent (the solver's `cost` includes preference
 * penalties that aren't real time). Flat speed + ascent rate vary by
 * travel mode; the ascent term is skipped when gain is unknown.
 */
function naismithMinutes(lengthM: number, gainM: number | null, profile: Profile): number {
  const flatKmh = profile === "bicycle" ? 14 : profile === "ski" ? 8 : 4.5;
  const ascentMPerH = profile === "bicycle" ? 400 : profile === "ski" ? 500 : 600;
  const flatMin = (lengthM / 1000 / flatKmh) * 60;
  const climbMin = gainM != null ? (gainM / ascentMPerH) * 60 : 0;
  return flatMin + climbMin;
}

/** "45 min" / "2 h 05 min". */
function formatDuration(min: number): string {
  const total = Math.max(1, Math.round(min));
  if (total < 60) return `${total} min`;
  const h = Math.floor(total / 60);
  const m = total % 60;
  return m === 0 ? `${h} h` : `${h} h ${String(m).padStart(2, "0")} min`;
}

/// Compact distance label: metres under 1 km, else km to 2 dp.
function fmtDist(m: number): string {
  return m >= 1000 ? `${(m / 1000).toFixed(2)} km` : `${Math.round(m)} m`;
}

function RangeRow({
  label,
  value,
  setValue,
  min,
  max,
  step,
  hint,
  format,
}: {
  label: string;
  value: number;
  setValue: (v: number) => void;
  min: number;
  max: number;
  step: number;
  hint?: string;
  format?: (v: number) => string;
}) {
  return (
    <label className="text-sm block">
      <div className="flex items-center gap-2">
        <code className="text-xs bg-ink-100 px-1 py-0.5 rounded w-32 inline-block">
          {label}
        </code>
        <input
          type="range"
          min={min}
          max={max}
          step={step}
          value={value}
          onChange={(e) => setValue(Number(e.target.value))}
          className="flex-1"
        />
        <span className="text-xs text-ink-500 w-14 text-right">
          {format ? format(value) : value}
        </span>
      </div>
      {hint ? <div className="text-xs text-ink-500 mt-1 ml-32">{hint}</div> : null}
    </label>
  );
}

function ResultPanel({ resp }: { resp: PathfindResp }) {
  const p = resp.path;
  const strategyColor =
    p.strategy === "on_graph"
      ? "text-blue-700 bg-blue-50 border-blue-200"
      : p.strategy === "hybrid"
        ? "text-violet-700 bg-violet-50 border-violet-200"
        : "text-orange-700 bg-orange-50 border-orange-200";
  return (
    <section className="space-y-3 border-t border-ink-200 pt-4">
      <div className="text-xs uppercase tracking-wide text-ink-500">Result</div>
      <div className={`inline-block text-xs font-mono px-2 py-1 rounded border ${strategyColor}`}>
        strategy: {p.strategy}
      </div>
      <table className="text-sm w-full">
        <tbody>
          <tr>
            <td className="text-ink-500 pr-3 py-0.5">length</td>
            <td className="font-mono">{p.length_m.toFixed(0)} m</td>
          </tr>
          <tr>
            <td className="text-ink-500 pr-3 py-0.5">cost</td>
            <td className="font-mono">{p.cost.toFixed(1)}</td>
          </tr>
          <tr>
            <td className="text-ink-500 pr-3 py-0.5">on-trail</td>
            <td className="font-mono">{p.on_trail_pct.toFixed(1)} %</td>
          </tr>
          <tr>
            <td className="text-ink-500 pr-3 py-0.5">solver</td>
            <td className="font-mono">{resp.took_us} µs</td>
          </tr>
        </tbody>
      </table>
      {p.fkb_breakdown && Object.keys(p.fkb_breakdown).length > 0 ? (
        <div>
          <div className="text-xs text-ink-500 mb-1">Surface breakdown</div>
          <table className="text-xs w-full" data-testid="fkb-breakdown">
            <tbody>
              {Object.entries(p.fkb_breakdown)
                .sort((a, b) => b[1] - a[1])
                .map(([kind, m]) => (
                  <tr key={kind} data-testid={`fkb-row-${kind}`}>
                    <td className="text-ink-500 py-0.5">{kind}</td>
                    <td className="font-mono py-0.5">{m.toFixed(0)} m</td>
                    <td className="font-mono py-0.5 text-ink-500">
                      {p.length_m > 0
                        ? ((m / p.length_m) * 100).toFixed(0)
                        : "0"}{" "}
                      %
                    </td>
                  </tr>
                ))}
            </tbody>
          </table>
        </div>
      ) : null}
      {p.legs.length > 1 ? (
        <div>
          <div className="text-xs text-ink-500 mb-1">Legs</div>
          <table className="text-xs w-full">
            <tbody>
              {p.legs.map((leg, i) => (
                <tr key={i}>
                  <td className="py-0.5">
                    <span
                      className="inline-block w-3 h-3 rounded mr-2 align-middle"
                      style={{ backgroundColor: LEG_COLOR[leg.kind] }}
                    />
                  </td>
                  <td className="font-mono py-0.5">{leg.kind}</td>
                  <td className="font-mono py-0.5 text-right">
                    {leg.length_m.toFixed(0)} m
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : null}
      {p.refused_by.length > 0 ? (
        <div className="text-xs text-amber-800 bg-amber-50 border border-amber-200 rounded p-2">
          Cells refused by:{" "}
          {p.refused_by.map((r) => (
            <code key={r} className="bg-amber-100 px-1 rounded mr-1">
              {r}
            </code>
          ))}
        </div>
      ) : null}
    </section>
  );
}
