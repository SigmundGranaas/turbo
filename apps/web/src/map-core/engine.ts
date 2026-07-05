/** The feature-facing map engine contract.
 *
 *  This is the narrow set of camera/projection/scene verbs that features, the
 *  host, and overlays use — a hand-written interface that the WASM `TurboMap`
 *  satisfies structurally (so features never name the concrete WASM type, and
 *  it can be stubbed in unit tests). The renderer/tile lifecycle (`render`,
 *  `apply_scene`, `ingest_*`, `streaming_plan`) is deliberately NOT here — that
 *  is `map-engine`-internal, not something a feature should reach for.
 *
 *  Signatures mirror `turbomap-web`'s generated `.d.ts`; if the bindings drift,
 *  the `publish(map)` call in `MapSurface` fails to type-check, which is the
 *  intended early-warning. */
export interface MapEngine {
  /** Geo → screen px (DPR-scaled). `undefined` when off-screen / behind camera. */
  project(lat: number, lng: number): Float64Array | undefined;
  /** Screen px → ground-plane geo (pitch-consistent). `undefined` if no hit. */
  unproject(x: number, y: number): Float64Array | undefined;
  /** Screen px → geo, raycast against the 3D relief (not the flat sea-level
   *  plane `unproject` uses). Returns `[lat, lng, hitTerrain]` — `hitTerrain` is
   *  1 when the ray struck terrain, 0 on the flat fallback (2D / no DEM). Use
   *  this for clicks, marker placement, and waypoint drag so they land where the
   *  user sees in a tilted 3D view. */
  unproject_ground(x: number, y: number): Float64Array;

  ease_to(lat: number, lng: number, zoom: number, bearing_deg: number, duration_ms: number): void;
  set_camera(lat: number, lng: number, zoom: number, pitch_deg: number, bearing_deg: number): void;
  camera_json(): string;

  pan_by_pixels(dx: number, dy: number): void;
  zoom_around(factor: number, fx: number, fy: number): void;
  zoom_around_animated(factor: number, fx: number, fy: number, duration_ms: number): void;
  orbit_around(d_bearing_deg: number, d_pitch_deg: number, fx: number, fy: number): void;
  fling(vx: number, vy: number): void;
  zoom_fling(zoom_velocity: number, fx: number, fy: number): void;

  set_viewport_inset(bottom_px: number): void;
  set_viewport_inset_right(right_px: number): void;

  set_sun_time(unix_secs?: number | null): void;
  set_terrain_shadows(strength: number): void;
  set_basemap_gain(gain: number): void;
  /** Toggle 3D terrain sun-lighting. `false` = bare bright basemap over the
   *  relief (no darkening, no per-fragment shading) — used for plain 3D; `true`
   *  for sun mode. No effect in 2D. */
  set_terrain_lit(lit: boolean): void;
  /** Toggle far-distance atmospheric haze (aerial perspective) in 3D. Off keeps
   *  the map crisp at every angle/zoom. No effect in 2D. */
  set_aerial_haze(on: boolean): void;
}
