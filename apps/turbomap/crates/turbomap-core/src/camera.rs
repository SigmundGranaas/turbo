//! Camera model. `Camera` is a value type; intent functions (`pan_by_pixels`,
//! `zoom_around`) translate platform pixel input into camera state changes.
//!
//! The renderer assumes 256-pixel tiles, so one world unit = `256 * 2^zoom`
//! pixels at the screen. Continuous zoom is supported (the tile pyramid level
//! used for sampling is independent and lives elsewhere).
//!
//! `CameraAnimation` is the value-typed companion for smooth transitions —
//! it's held by `Scene` and ticked at frame time, never inside `Camera`
//! itself.

use std::time::Duration;

// `web_time::Instant` is `std::time::Instant` on native and a `performance.now()`
// shim on wasm32 (std's `Instant::now()` panics in the browser). Camera animation
// timestamps cross into the engine, so the whole stack must use the same type.
use web_time::Instant;

use glam::{Mat3, Mat4, Vec3};

use crate::geo::{LatLng, WorldPoint, MAX_LATITUDE_DEG};

/// One tile is `TILE_SIZE_PX` pixels wide at its native zoom.
pub const TILE_SIZE_PX: f64 = 256.0;

/// Vertical field of view used for the perspective projection. Matches
/// the MapLibre default. With `altitude` chosen as
/// `(vh / 2) / ppw / tan(fov_y / 2)` the perspective collapses to the
/// legacy 2D orthographic transform when `pitch_deg == 0` — pixel-
/// exact match, no spec change for callers that don't tilt.
const FOV_Y: f32 = 0.6435; // ~36.87°
const MAX_PITCH_DEG: f64 = 80.0;

/// Earth-curvature drop coefficient for the terrain mesh, in this engine's
/// anisotropic world units: a ground point `s` horizontal world-units from the
/// camera centre sits `coeff · s²` *below* the tangent plane (in vertical world
/// units). Physically `Δz_m = s_m²/2R`; folding both Mercator scales
/// (horizontal `s_m = s·C·cosφ`, vertical `Δz_world = Δz_m·cosφ/C`) collapses
/// the earth radius out entirely, leaving `π·cos³φ`. The terrain WGSL applies
/// it so the distant ground bends away instead of standing on a flat disc.
pub fn earth_curvature_coeff(coslat: f32) -> f32 {
    std::f32::consts::PI * coslat.abs().powi(3)
}

/// Horizontal distance (world units) from the camera nadir to the true ground
/// horizon, for an eye at the given `altitude_world` view-ray length and
/// `pitch_rad`. Inverse of [`earth_curvature_coeff`]: the horizon is where the
/// curvature drop equals the eye height, i.e. `√(altitude·cos(pitch) / (π·cos³φ))`
/// (radius again cancels). Used to push the far clip plane out so far terrain
/// isn't clipped before it dissolves into the atmosphere.
pub fn ground_horizon_world(altitude_world: f32, pitch_rad: f32, coslat: f32) -> f32 {
    let eye_h = altitude_world.max(0.0) * pitch_rad.cos().max(0.0);
    (eye_h / earth_curvature_coeff(coslat).max(1e-9)).sqrt()
}

/// Default zoom bounds. z0 is the whole world in one root tile; z24 is
/// deeper than any tile source serves but lets the camera glide smoothly to
/// the limit when no tighter limit is configured. A [`Camera`] clamps to
/// these unless given its own [`ZoomBounds`] (which the [`Map`](crate::Map)
/// derives from the active tile sources so you cannot zoom past where real
/// tiles exist).
pub const MIN_ZOOM: f64 = 0.0;
pub const MAX_ZOOM: f64 = 24.0;

/// An `f64` proven finite (no `NaN`/`Inf`) — the parse-don't-validate
/// primitive for values crossing the untrusted host boundary (the FFI, a
/// deserialized scene, a bad animation sample). You can't *construct* one
/// from a `NaN`, so holding a `FiniteF64` is proof the value is safe to do
/// projection math with. Use [`FiniteF64::or`] to coerce-with-fallback at a
/// boundary that must not reject the whole update.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FiniteF64(f64);

impl FiniteF64 {
    /// `Some` iff `v` is finite. The only constructor — parse, don't validate.
    pub fn new(v: f64) -> Option<Self> {
        v.is_finite().then_some(Self(v))
    }

    /// `v` if finite, otherwise `fallback`. For sanitising untrusted input
    /// in place without discarding the surrounding update.
    pub fn or(v: f64, fallback: f64) -> f64 {
        if v.is_finite() {
            v
        } else {
            fallback
        }
    }

    pub fn get(self) -> f64 {
        self.0
    }
}

/// The inclusive `[min, max]` zoom range a camera may occupy. Constructed
/// from the active map sources' supported zoom (so the user cannot zoom
/// past the map's accuracy and watch the raster blur out from under sharp,
/// f64-projected overlays), or set explicitly by the host.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZoomBounds {
    pub min: f64,
    pub max: f64,
}

impl ZoomBounds {
    /// The renderer-wide default, [`MIN_ZOOM`]..=[`MAX_ZOOM`].
    pub const DEFAULT: ZoomBounds = ZoomBounds {
        min: MIN_ZOOM,
        max: MAX_ZOOM,
    };

    /// A bound from an explicit range. The inputs are ordered (so
    /// `new(10, 4)` is the same as `new(4, 10)`) and each end is clamped
    /// into the absolute `[MIN_ZOOM, MAX_ZOOM]` envelope so a degenerate
    /// request can never push the camera somewhere the tile math can't
    /// represent.
    pub fn new(min: f64, max: f64) -> Self {
        let lo = min.min(max).clamp(MIN_ZOOM, MAX_ZOOM);
        let hi = min.max(max).clamp(MIN_ZOOM, MAX_ZOOM);
        ZoomBounds { min: lo, max: hi }
    }

    /// Clamp a zoom level into this range.
    pub fn clamp(self, zoom: f64) -> f64 {
        zoom.clamp(self.min, self.max)
    }

    /// The bounds implied by a set of tile sources' `[min_zoom, max_zoom]`
    /// ranges: min is the shallowest any source serves, max is the deepest
    /// served level plus an overzoom budget ([`OVERZOOM_LEVELS`]) so the
    /// camera can dip slightly past the sharpest tile before it's purely
    /// upsampling. An empty set falls back to [`ZoomBounds::DEFAULT`].
    pub fn from_sources(ranges: impl IntoIterator<Item = (u8, u8)>) -> Self {
        let mut union: Option<(u8, u8)> = None;
        for (min_z, max_z) in ranges {
            union = Some(match union {
                Some((lo, hi)) => (lo.min(min_z), hi.max(max_z)),
                None => (min_z, max_z),
            });
        }
        match union {
            Some((lo, hi)) => ZoomBounds::new(lo as f64, hi as f64 + OVERZOOM_LEVELS),
            None => ZoomBounds::DEFAULT,
        }
    }
}

impl Default for ZoomBounds {
    fn default() -> Self {
        ZoomBounds::DEFAULT
    }
}

/// How far past the deepest real tile the camera may zoom before it's just
/// upsampling. Added to the source-derived max by [`ZoomBounds::from_sources`].
pub const OVERZOOM_LEVELS: f64 = 3.0;

/// The camera's zoom lock: the range the camera is clamped to, plus an
/// optional host override.
///
/// Replaces the `Map`'s old `zoom_bounds: ZoomBounds` + `manual_zoom_bounds:
/// Option<ZoomBounds>` pair — two fields encoding one decision. Without an
/// override the [`active`](Self::active) range tracks the union of the active
/// tile sources (so the user can't zoom past the map's accuracy); with one,
/// the host's range wins. The "which applies?" question has a single answer:
/// re-[`resolve`](Self::resolve) and read [`active`](Self::active).
#[derive(Debug, Clone, Copy)]
pub struct ZoomLock {
    /// Host override. `Some` wins over the source-derived range.
    manual: Option<ZoomBounds>,
    /// The range the camera is currently clamped to.
    active: ZoomBounds,
}

impl ZoomLock {
    /// A lock starting at `initial`, tracking sources (no override).
    pub fn new(initial: ZoomBounds) -> Self {
        Self {
            manual: None,
            active: initial,
        }
    }

    /// The range the camera is currently locked to.
    pub fn active(&self) -> ZoomBounds {
        self.active
    }

    /// Set or clear the host override. Call [`resolve`](Self::resolve)
    /// afterwards to recompute and apply the active range.
    pub fn set_manual(&mut self, bounds: Option<ZoomBounds>) {
        self.manual = bounds;
    }

    /// Recompute the active range: a host override wins; otherwise the union
    /// of the given source ranges via [`ZoomBounds::from_sources`]. Returns
    /// the new active range.
    pub fn resolve(&mut self, source_ranges: impl IntoIterator<Item = (u8, u8)>) -> ZoomBounds {
        self.active = self
            .manual
            .unwrap_or_else(|| ZoomBounds::from_sources(source_ranges));
        self.active
    }
}

/// Camera state. `f64` zoom is continuous.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub center: LatLng,
    pub zoom: f64,
    /// Tilt away from top-down, degrees. 0 = pure top-down (orthographic-
    /// equivalent perspective), 60 = highly tilted. Clamped on use.
    pub pitch_deg: f64,
    /// Compass bearing of the up direction on screen, degrees.
    /// 0 = north up, 90 = east up, ... Wraps to [0, 360).
    pub bearing_deg: f64,
    /// Bottom viewport inset in pixels (e.g. a sheet over the lower map). Shifts
    /// the projection's principal point up by `inset/2`, so the camera centre and
    /// everything projected/rendered sits in the visible band above the inset.
    /// Default 0 (no inset) — the offscreen/golden path never sets it, so its
    /// projection + matrix are unchanged.
    pub viewport_inset_px: f64,
    /// Right viewport inset in pixels (e.g. a desktop side panel over the right
    /// of the map). Shifts the projection's principal point left by `inset/2`,
    /// so the camera centre + everything projected/rendered sits in the visible
    /// band left of the panel. Default 0 (no inset) — golden path unchanged.
    pub viewport_inset_right_px: f64,
    /// The zoom range this camera may occupy. Interactive zoom
    /// ([`zoomed_around`](Self::zoomed_around)) clamps to it, so the camera
    /// can be locked to the active map's accuracy. Defaults to
    /// [`ZoomBounds::DEFAULT`]; the offscreen/golden path leaves it at the
    /// default so its clamp behaviour is unchanged.
    pub zoom_bounds: ZoomBounds,
}

impl Camera {
    pub const fn new(center: LatLng, zoom: f64) -> Self {
        Self {
            center,
            zoom,
            pitch_deg: 0.0,
            bearing_deg: 0.0,
            viewport_inset_px: 0.0,
            viewport_inset_right_px: 0.0,
            zoom_bounds: ZoomBounds::DEFAULT,
        }
    }

    /// Coerce every field into its valid, finite domain — the sanitising
    /// boundary for a camera pose arriving from an untrusted source (the host
    /// FFI, a deserialized scene). A `NaN`/`Inf` centre, zoom, pitch, bearing
    /// or inset is replaced with a safe value rather than propagated into the
    /// projection, where it would produce the `NaN` matrix a mobile GPU driver
    /// hangs on (see `Map::render`'s finite gate — this is the upstream half
    /// of that defense: stop the bad value at the seam, and catch any that
    /// still slip through at the GPU).
    ///
    /// Coercions: non-finite → safe default; latitude clamped to the Mercator
    /// envelope; zoom clamped to `[MIN_ZOOM, MAX_ZOOM]`; pitch to
    /// `[0, MAX_PITCH_DEG]`; bearing wrapped to `[0, 360)`; inset ≥ 0.
    pub fn sanitized(self) -> Self {
        // Wrap a helper that also bounds the magnitude: a *finite* value can
        // still be absurd (e.g. 1e177), and anything past `f32::MAX` becomes
        // `Inf` the moment the projection casts it to f32 — the same driver
        // hazard as a raw NaN. Every field below is brought into a domain
        // whose downstream f32 cast is comfortably finite.
        let wrap_360 = |v: f64| {
            // `rem_euclid` handles negatives; the `>= 360` guard catches the
            // float-rounding edge where a tiny-negative input rounds up to
            // exactly 360.0 (which must read as 0, not escape the range).
            let b = FiniteF64::or(v, 0.0).rem_euclid(360.0);
            if b >= 360.0 || !b.is_finite() {
                0.0
            } else {
                b
            }
        };
        let lat = FiniteF64::or(self.center.lat, 0.0).clamp(-MAX_LATITUDE_DEG, MAX_LATITUDE_DEG);
        // A camera centre is a point on the globe — normalise longitude into
        // [-180, 180) so an over-pan (or a wild host value) can't push world-x
        // out of f32 range.
        let lng = {
            let l = (FiniteF64::or(self.center.lng, 0.0) + 180.0).rem_euclid(360.0) - 180.0;
            // rem_euclid keeps it in [-180, 180); guard the exact-360 edge.
            if l.is_finite() {
                l
            } else {
                0.0
            }
        };
        Self {
            center: LatLng::new(lat, lng),
            zoom: FiniteF64::or(self.zoom, MIN_ZOOM).clamp(MIN_ZOOM, MAX_ZOOM),
            pitch_deg: FiniteF64::or(self.pitch_deg, 0.0).clamp(0.0, MAX_PITCH_DEG),
            bearing_deg: wrap_360(self.bearing_deg),
            // No real viewport/sheet inset approaches 100k px; clamp there so a
            // garbage value can't shift the projection's principal point out of
            // f32 range.
            viewport_inset_px: FiniteF64::or(self.viewport_inset_px, 0.0).clamp(0.0, 100_000.0),
            viewport_inset_right_px: FiniteF64::or(self.viewport_inset_right_px, 0.0)
                .clamp(0.0, 100_000.0),
            zoom_bounds: self.zoom_bounds,
        }
    }

    /// Set the zoom range the camera may occupy and clamp the current zoom
    /// into it. The lock that stops the user zooming past the map's
    /// accuracy.
    pub fn with_zoom_bounds(mut self, bounds: ZoomBounds) -> Self {
        self.zoom_bounds = bounds;
        self.zoom = bounds.clamp(self.zoom);
        self
    }

    /// Replace the zoom bounds in place, clamping the current zoom into the
    /// new range. Used by [`Map`](crate::Map) to keep the camera locked to
    /// its sources as layers change.
    pub fn set_zoom_bounds(&mut self, bounds: ZoomBounds) {
        self.zoom_bounds = bounds;
        self.zoom = bounds.clamp(self.zoom);
    }

    pub fn with_pitch(mut self, deg: f64) -> Self {
        self.pitch_deg = deg;
        self
    }

    /// Set the bottom viewport inset (px). See [`Camera::viewport_inset_px`].
    pub fn with_viewport_inset(mut self, bottom_px: f64) -> Self {
        self.viewport_inset_px = bottom_px.max(0.0);
        self
    }

    /// Set the right viewport inset (px). See [`Camera::viewport_inset_right_px`].
    pub fn with_viewport_inset_right(mut self, right_px: f64) -> Self {
        self.viewport_inset_right_px = right_px.max(0.0);
        self
    }

    pub fn with_bearing(mut self, deg: f64) -> Self {
        self.bearing_deg = deg;
        self
    }

    /// Rotate the compass bearing by `delta_deg` (clockwise positive), the
    /// two-finger rotate gesture. Wraps to `[0, 360)` so repeated spins
    /// never drift the value out of range. Pivots about the screen centre.
    pub fn rotate_by(&mut self, delta_deg: f64) {
        self.bearing_deg = (self.bearing_deg + delta_deg).rem_euclid(360.0);
    }

    /// Tilt by `delta_deg` (two-finger vertical drag), clamped to
    /// `[0, MAX_PITCH_DEG]`. Pivots about the screen centre.
    pub fn pitch_by(&mut self, delta_deg: f64) {
        self.pitch_deg = (self.pitch_deg + delta_deg).clamp(0.0, MAX_PITCH_DEG);
    }

    /// Rotate the bearing by `delta_deg` about `focus_px` (the two-finger
    /// rotate centroid), keeping that pixel over the same world point —
    /// the natural pivot for a two-finger gesture. `rotate_by` is this with
    /// the focus at the screen centre.
    pub fn rotated_around(
        mut self,
        delta_deg: f64,
        focus_px: (f64, f64),
        viewport_px: (f64, f64),
    ) -> Camera {
        let before = self.pixel_to_world(focus_px, viewport_px);
        self.bearing_deg = (self.bearing_deg + delta_deg).rem_euclid(360.0);
        self.recenter_on_focus(before, focus_px, viewport_px);
        self
    }

    /// Tilt by `delta_deg` (clamped) about `focus_px`, keeping that pixel
    /// over the same world point.
    pub fn pitched_around(
        mut self,
        delta_deg: f64,
        focus_px: (f64, f64),
        viewport_px: (f64, f64),
    ) -> Camera {
        let before = self.pixel_to_world(focus_px, viewport_px);
        self.pitch_deg = (self.pitch_deg + delta_deg).clamp(0.0, MAX_PITCH_DEG);
        self.recenter_on_focus(before, focus_px, viewport_px);
        self
    }

    /// Shift the centre so `focus_world` lands back under `focus_px` after a
    /// projection-changing edit (rotate/tilt). Shared focus-invariance step.
    fn recenter_on_focus(
        &mut self,
        focus_world: WorldPoint,
        focus_px: (f64, f64),
        viewport_px: (f64, f64),
    ) {
        let after = self.pixel_to_world(focus_px, viewport_px);
        let c = self.center.to_world();
        self.center =
            WorldPoint::new(c.x - (after.x - focus_world.x), c.y - (after.y - focus_world.y))
                .to_lat_lng();
    }

    /// Pixels per world unit at the current zoom (one world unit = the full
    /// Mercator extent, i.e. one root tile). This is the value at the
    /// camera's centre on the ground plane — perspective makes the
    /// effective ppw vary with screen position when pitched.
    pub fn pixels_per_world_unit(self) -> f64 {
        TILE_SIZE_PX * 2.0_f64.powf(self.zoom)
    }

    /// Build the 4×4 view–projection matrix the vertex shaders use to
    /// place a world-space point in clip space. For `pitch=0,
    /// bearing=0` this collapses to the legacy 2D transform (verified
    /// by `projection_at_pitch_zero_matches_legacy_ortho`).
    pub fn view_projection_matrix(self, viewport_px: (u32, u32)) -> [[f32; 4]; 4] {
        self.view_projection(viewport_px).to_cols_array_2d()
    }

    /// Like [`Self::view_projection_matrix`] but built in a frame translated so
    /// the f64 world point `origin` maps to the world-space zero. Vertex
    /// pipelines pass `(world - origin)` as f32 (small magnitudes → full f32
    /// precision) instead of absolute Mercator coords near 0.5 (which lose
    /// ~16 px at zoom ≈ 20). The matrix is otherwise identical, so with
    /// `origin = (0,0)` this equals `view_projection_matrix`.
    pub fn view_projection_matrix_rtc(
        self,
        origin: WorldPoint,
        viewport_px: (u32, u32),
    ) -> [[f32; 4]; 4] {
        self.view_projection_from_origin(origin, viewport_px)
            .to_cols_array_2d()
    }

    fn view_projection(self, viewport_px: (u32, u32)) -> Mat4 {
        // Absolute frame: origin at world (0,0). Kept for the legacy/ortho
        // parity tests; live pipelines use the RTC path.
        self.view_projection_from_origin(WorldPoint::new(0.0, 0.0), viewport_px)
    }

    /// Height of the camera eye above the z=0 ground plane, in world
    /// units, for the given viewport. The eye sits at `altitude` along the
    /// pitched view ray, so its vertical component is `altitude·cos(pitch)`
    /// — used to keep the camera from dropping below the 3D terrain.
    pub fn eye_world_z(self, viewport_px: (u32, u32)) -> f32 {
        let vh = viewport_px.1.max(1) as f32;
        let ppw = self.pixels_per_world_unit() as f32;
        let altitude = (vh * 0.5) / ppw / (FOV_Y * 0.5).tan();
        let pitch = (self.pitch_deg.clamp(0.0, MAX_PITCH_DEG) as f32).to_radians();
        altitude * pitch.cos()
    }

    /// The full camera altitude (eye distance along the view ray) for the
    /// given viewport — independent of pitch. Used to size a terrain
    /// clearance margin.
    pub fn altitude_world(self, viewport_px: (u32, u32)) -> f32 {
        let vh = viewport_px.1.max(1) as f32;
        let ppw = self.pixels_per_world_unit() as f32;
        (vh * 0.5) / ppw / (FOV_Y * 0.5).tan()
    }

    /// Camera eye position relative to the look-at centre, in world units,
    /// for the given viewport. This is the eye in the same relative-to-centre
    /// (RTC) frame the terrain shaders emit (their origin is `center`), so the
    /// fragment shader can compute true eye→fragment distance for physically
    /// based aerial perspective. Mirrors the eye construction in
    /// `view_projection_from_origin` with `origin == center` (target at 0).
    pub fn eye_offset_world(self, viewport_px: (u32, u32)) -> [f32; 3] {
        let vh = viewport_px.1.max(1) as f32;
        let ppw = self.pixels_per_world_unit() as f32;
        let altitude = (vh * 0.5) / ppw / (FOV_Y * 0.5).tan();
        let pitch = (self.pitch_deg.clamp(0.0, MAX_PITCH_DEG) as f32).to_radians();
        let bearing = (self.bearing_deg as f32).to_radians();
        let pitch_rot = Mat3::from_rotation_x(-pitch);
        let bearing_rot = Mat3::from_rotation_z(bearing);
        let eye = bearing_rot * (pitch_rot * Vec3::new(0.0, 0.0, altitude));
        [eye.x, eye.y, eye.z]
    }

    fn view_projection_from_origin(self, origin: WorldPoint, viewport_px: (u32, u32)) -> Mat4 {
        let vw = viewport_px.0.max(1) as f32;
        let vh = viewport_px.1.max(1) as f32;
        let ppw = self.pixels_per_world_unit() as f32;
        let altitude = (vh * 0.5) / ppw / (FOV_Y * 0.5).tan();

        let pitch = (self.pitch_deg.clamp(0.0, MAX_PITCH_DEG) as f32).to_radians();
        let bearing = (self.bearing_deg as f32).to_radians();

        // Build target/eye in the origin-relative frame: subtract the f64
        // origin in f64, then cast — so the f32 the matrix carries is the small
        // offset from the origin, not the ~0.5 absolute Mercator coordinate.
        let centre = self.center.to_world();
        let target = Vec3::new((centre.x - origin.x) as f32, (centre.y - origin.y) as f32, 0.0);

        // Eye position relative to target.
        //   1. Start at (0, 0, altitude) — straight above the target.
        //   2. Pitch backward by rotating about the X axis (east). The
        //      eye moves south (+y) and lower.
        //   3. Bearing rotates the (eye - target) vector clockwise
        //      around z (viewed from above) so heading B becomes
        //      screen-up.
        // Pitch tilts the camera away from straight-down. We need the
        // eye to move toward the *south* side of the target (so the
        // camera looks northward) and slightly downward. World +y is
        // south here, so a positive south-shift means +y. The natural
        // `Mat3::from_rotation_x(+pitch)` applied to (0,0,alt)
        // produces -y — i.e. the *north* side of the target — which
        // makes the rendered horizon appear at the BOTTOM of the
        // screen and screen-up points underground. Flipping the
        // pitch sign in the rotation puts the eye on the correct
        // side; screen-up then rotates from world-north (at pitch=0)
        // toward world-sky (at pitch=90) as expected.
        let pitch_rot = Mat3::from_rotation_x(-pitch);
        // Bearing rotates the camera CCW around +z (right-hand rule,
        // visually clockwise when viewed from above) so that compass
        // heading B lands at screen-top. The map appears to rotate
        // clockwise as the user dials bearing up.
        let bearing_rot = Mat3::from_rotation_z(bearing);
        let eye_offset = bearing_rot * (pitch_rot * Vec3::new(0.0, 0.0, altitude));
        let eye = target + eye_offset;

        // Up hint: world-north (-y) rotated by bearing. Stays in the
        // ground plane so lookAt's Gram-Schmidt remains stable even
        // at pitch=0 (where the true camera-up is +z, collinear with
        // the look direction).
        let up_north = Vec3::new(0.0, -1.0, 0.0);
        let up = bearing_rot * up_north;

        // Use LEFT-HANDED view + perspective. Our world has +y =
        // south (Mercator convention), so the natural map orientation
        // (north up, east right, viewer above) is left-handed in that
        // frame. Trying to express it with `look_at_rh` either flips
        // x (east lands on screen-left) or up (south lands at top).
        // `look_at_lh` with up_hint = north (-y) makes the camera
        // basis come out as right=east, up=north, back=+z — the map
        // appears the way users expect.
        let view = Mat4::look_at_lh(eye, target, up);

        // Near/far chosen relative to altitude so the depth range
        // adapts to the current zoom without precision loss. When pitched,
        // the far plane is also pushed out to the true ground horizon so far
        // terrain isn't clipped before it can dissolve into the atmosphere
        // (the old altitude·100 cap clips the horizon at high zoom + grazing
        // pitch). Gated by pitch so the flat 2D map (pitch 0) is byte-identical.
        let aspect = vw / vh;
        let near = altitude * 0.01;
        let mut far = altitude * 100.0;
        if self.pitch_deg > 0.0 {
            let coslat = (self.center.lat.to_radians().cos() as f32).abs().max(1e-3);
            let horizon = ground_horizon_world(altitude, pitch, coslat);
            far = far.max(horizon * 1.25);
        }
        let proj = Mat4::perspective_lh(FOV_Y, aspect, near, far);

        // Bottom inset: shift content up by inset/2 px. In NDC (height 2 over vh
        // px), that's +inset/vh on y. Left-multiplying a clip-space translation
        // adds `t·w` to clip.y, i.e. ndc.y += t after the perspective divide.
        // inset=0 → identity translation → matrix byte-identical (goldens hold).
        if self.viewport_inset_px == 0.0 && self.viewport_inset_right_px == 0.0 {
            proj * view
        } else {
            // Bottom inset shifts content up by inset/2 px (+y NDC); a right
            // inset shifts content left by inset/2 px (−x NDC). NDC spans 2 over
            // the viewport, so px→NDC is `inset/dim`.
            let ndc_shift_y = (self.viewport_inset_px / vh as f64) as f32;
            let ndc_shift_x = -(self.viewport_inset_right_px / vw as f64) as f32;
            Mat4::from_translation(Vec3::new(ndc_shift_x, ndc_shift_y, 0.0)) * proj * view
        }
    }

    /// Shift the camera by a pixel delta. Positive `dx` moves the *map*
    /// rightwards under the user's finger — i.e. the camera moves *left*.
    /// Bearing rotates the pan direction so dragging right always moves
    /// the map right *on screen* regardless of compass heading.
    pub fn pan_by_pixels(&mut self, dx: f64, dy: f64) {
        let ppw = self.pixels_per_world_unit();
        let bearing_rad = self.bearing_deg.to_radians();
        let (s, c) = bearing_rad.sin_cos();
        // Rotate screen-space delta into world-space delta. Per the
        // screen→world basis at bearing B:
        //   screen_right = (cos B,  sin B)  in world coords
        //   screen_down  = (-sin B, cos B)
        // So a screen delta (dx, dy) maps to world delta
        //   (dx * cos B - dy * sin B,  dx * sin B + dy * cos B).
        // The camera moves opposite so the map stays under the user's
        // finger.
        let dx_world = (dx * c - dy * s) / ppw;
        let dy_world = (dx * s + dy * c) / ppw;
        let center_world = self.center.to_world();
        let new_world = WorldPoint::new(center_world.x - dx_world, center_world.y - dy_world);
        self.center = new_world.to_lat_lng();
    }

    /// Project a world-space point on the ground plane to a screen pixel.
    /// Returns `None` if the point lands behind the camera (only possible
    /// when pitched far enough that the world point is past the horizon).
    /// Used by the text + marker pipelines to position screen-aligned
    /// labels and circles at world anchors.
    pub fn world_to_screen(
        self,
        world: WorldPoint,
        viewport_px: (f64, f64),
    ) -> Option<(f64, f64)> {
        // Bottom inset shifts the principal point up by inset/2 px (so the camera
        // centre lands in the visible band above the inset). Default 0 = no shift.
        let inset_y = self.viewport_inset_px * 0.5;
        let inset_x = self.viewport_inset_right_px * 0.5;
        if self.pitch_deg == 0.0 && self.bearing_deg == 0.0 {
            let ppw = self.pixels_per_world_unit();
            let centre = self.center.to_world();
            return Some((
                (world.x - centre.x) * ppw + viewport_px.0 * 0.5 - inset_x,
                (world.y - centre.y) * ppw + viewport_px.1 * 0.5 - inset_y,
            ));
        }
        // RTC frame (translated to the camera centre) so the f32 matrix carries
        // small origin-relative magnitudes — same high-zoom precision fix as
        // `pixel_to_world`; keeps overlays/markers from shimmering under tilt.
        let origin = self.center.to_world();
        let vp =
            self.view_projection_from_origin(origin, (viewport_px.0 as u32, viewport_px.1 as u32));
        let clip = vp
            * glam::Vec4::new(
                (world.x - origin.x) as f32,
                (world.y - origin.y) as f32,
                0.0,
                1.0,
            );
        if clip.w <= 0.0 {
            return None;
        }
        let ndc = clip.truncate() / clip.w;
        // The matrix already applied the inset (NDC shift), so map NDC→px straight.
        Some((
            ((ndc.x + 1.0) * 0.5) as f64 * viewport_px.0,
            ((1.0 - ndc.y) * 0.5) as f64 * viewport_px.1,
        ))
    }

    /// Project a world point that sits at world-space height `world_z`
    /// (the same displaced-z the terrain mesh uses: `elev_m ·
    /// meters_to_world · exaggeration`) to screen pixels. This is what
    /// anchors markers/labels onto the 3D surface instead of the flat
    /// ground plane. Always uses the perspective RTC matrix (no z=0 fast
    /// path) since the whole point is the non-zero height.
    pub fn world_to_screen_z(
        self,
        world: WorldPoint,
        world_z: f32,
        viewport_px: (f64, f64),
    ) -> Option<(f64, f64)> {
        let origin = self.center.to_world();
        let vp =
            self.view_projection_from_origin(origin, (viewport_px.0 as u32, viewport_px.1 as u32));
        let clip = vp
            * glam::Vec4::new(
                (world.x - origin.x) as f32,
                (world.y - origin.y) as f32,
                world_z,
                1.0,
            );
        if clip.w <= 0.0 {
            return None;
        }
        let ndc = clip.truncate() / clip.w;
        Some((
            ((ndc.x + 1.0) * 0.5) as f64 * viewport_px.0,
            ((1.0 - ndc.y) * 0.5) as f64 * viewport_px.1,
        ))
    }

    /// Convert a screen pixel (relative to a viewport of `viewport_px`) to
    /// a world-space coordinate on the ground plane.
    ///
    /// At pitch=0, bearing=0 this matches the legacy orthographic
    /// formula. With tilt or bearing applied it unprojects the screen
    /// ray through the inverse view-projection matrix and intersects
    /// the z=0 plane. Returns the camera centre as a degenerate
    /// fallback when the ray is parallel to the ground (camera looking
    /// at the horizon).
    pub fn pixel_to_world(self, pixel: (f64, f64), viewport_px: (f64, f64)) -> WorldPoint {
        // Fast path that exactly matches the legacy formula — keeps
        // existing tests pixel-stable and avoids matrix inversion in
        // the common case.
        if self.pitch_deg == 0.0 && self.bearing_deg == 0.0 {
            let ppw = self.pixels_per_world_unit();
            let center = self.center.to_world();
            // Inverse of the inset-shifted forward projection (principal point up
            // by inset/2). The matrix path below inherits the inset for free via
            // the inset-aware `view_projection` it inverts.
            return WorldPoint::new(
                center.x + (pixel.0 - viewport_px.0 * 0.5 + self.viewport_inset_right_px * 0.5) / ppw,
                center.y + (pixel.1 - viewport_px.1 * 0.5 + self.viewport_inset_px * 0.5) / ppw,
            );
        }
        // Unproject in a frame translated to the camera centre (RTC): the f32
        // matrix then carries small origin-relative magnitudes instead of the
        // absolute ~0.5 Mercator coords, whose f32 quantisation (~6e-8 world ≈
        // several px at z16+) made the orbit re-pin jitter at high zoom. The
        // absolute coord is restored by adding the f64 origin back.
        let origin = self.center.to_world();
        let Some((near, dir)) = self.pixel_ray_from_origin(pixel, viewport_px, origin) else {
            return self.center.to_world();
        };
        if dir.z.abs() < 1e-6 {
            return self.center.to_world();
        }
        let t = -near.z / dir.z;
        let ground = near + dir * t;
        // A near-singular VP (extreme pitch/zoom) can make `inverse()` —
        // and thus `ground` — non-finite. Returning a NaN world point would
        // poison the camera centre on a zoom-around and feed NaN to the GPU
        // matrix (mobile-driver hang). Fall back to the camera centre.
        if !ground.x.is_finite() || !ground.y.is_finite() {
            return self.center.to_world();
        }
        WorldPoint::new(origin.x + ground.x as f64, origin.y + ground.y as f64)
    }

    /// The view ray for a screen pixel, in the `origin`-relative (RTC) frame.
    ///
    /// Returns `(near, dir)` where `near` is the point on the near plane and
    /// `dir = far - near` points into the scene — both relative to `origin`
    /// (add `origin` back for absolute world coords). `origin` should be the
    /// camera centre so the f32 matrix carries small offsets, matching
    /// [`pixel_to_world`] / [`world_to_screen_z`] precision. `None` when the
    /// inverse view-projection is non-finite (near-singular pitch/zoom).
    ///
    /// This is the shared ray construction behind both the flat-plane unproject
    /// ([`pixel_to_world`]) and the terrain raycast (`Map::screen_to_ground_lng_lat`),
    /// so they march an identical ray.
    pub(crate) fn pixel_ray_from_origin(
        self,
        pixel: (f64, f64),
        viewport_px: (f64, f64),
        origin: WorldPoint,
    ) -> Option<(Vec3, Vec3)> {
        let vp =
            self.view_projection_from_origin(origin, (viewport_px.0 as u32, viewport_px.1 as u32));
        let inv = vp.inverse();
        let ndc_x = (2.0 * pixel.0 / viewport_px.0 - 1.0) as f32;
        let ndc_y = (1.0 - 2.0 * pixel.1 / viewport_px.1) as f32;
        let near = inv.project_point3(Vec3::new(ndc_x, ndc_y, 0.0));
        let far = inv.project_point3(Vec3::new(ndc_x, ndc_y, 1.0));
        let dir = far - near;
        if !near.is_finite() || !dir.is_finite() {
            return None;
        }
        Some((near, dir))
    }

    /// Linear interpolation between two cameras. Centre is interpolated in
    /// world space (not lat/lng, which has nonlinear stretching near the
    /// poles), zoom + pitch + bearing linearly (bearing via shortest arc
    /// so 350° → 10° wraps the short way).
    pub fn lerp(self, target: Camera, t: f64) -> Camera {
        let a = self.center.to_world();
        let b = target.center.to_world();
        let world = WorldPoint::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
        let mut delta_bearing = target.bearing_deg - self.bearing_deg;
        // Shortest-arc bearing interpolation.
        if delta_bearing > 180.0 {
            delta_bearing -= 360.0;
        } else if delta_bearing < -180.0 {
            delta_bearing += 360.0;
        }
        Camera {
            center: world.to_lat_lng(),
            zoom: self.zoom + (target.zoom - self.zoom) * t,
            pitch_deg: self.pitch_deg + (target.pitch_deg - self.pitch_deg) * t,
            bearing_deg: (self.bearing_deg + delta_bearing * t).rem_euclid(360.0),
            // The inset is a viewport property, not a pose — keep it, don't interpolate.
            viewport_inset_px: self.viewport_inset_px,
            viewport_inset_right_px: self.viewport_inset_right_px,
            // Bounds are a constraint on the camera, not an interpolated pose
            // value; the start and target share them in practice (the Map
            // stamps both), so carry the start's.
            zoom_bounds: self.zoom_bounds,
        }
    }

    /// Zoom by a multiplicative `factor` (`>1` zooms in, `<1` zooms out)
    /// while keeping `focus_px` over the same world point. `viewport_px` is
    /// the viewport size in pixels.
    pub fn zoom_around(&mut self, factor: f64, focus_px: (f64, f64), viewport_px: (f64, f64)) {
        *self = self.zoomed_around(factor, focus_px, viewport_px);
    }

    /// Pure form of [`zoom_around`]: the camera after zooming by `factor`
    /// (multiplicative — 2.0 = one level in) about `focus_px`, with the
    /// focus pixel kept over the same world point. Zoom is clamped to the
    /// camera's [`ZoomBounds`] (the map-accuracy lock). Used both for the
    /// immediate setter and to
    /// compute the *target* of an animated double-tap / scroll zoom.
    pub fn zoomed_around(
        mut self,
        factor: f64,
        focus_px: (f64, f64),
        viewport_px: (f64, f64),
    ) -> Camera {
        // Reject non-finite + non-positive factors. A NaN factor slips past
        // a bare `factor <= 0.0` (NaN compares false), and `log2(NaN)` would
        // poison the zoom → a NaN view-projection → GPU driver hang on device.
        if !factor.is_finite() || factor <= 0.0 {
            return self;
        }
        let focus_world_before = self.pixel_to_world(focus_px, viewport_px);
        self.zoom = self.zoom_bounds.clamp(self.zoom + factor.log2());
        // After the (clamped) zoom change, the same focus pixel maps to a
        // different world point. Shift the centre so the focus world point
        // lands back under the focus pixel. With tilt we re-unproject
        // through the updated camera; without tilt the legacy direct math
        // is pixel-equivalent.
        if self.pitch_deg == 0.0 && self.bearing_deg == 0.0 {
            let ppw = self.pixels_per_world_unit();
            // Invert the SAME inset-shifted projection `pixel_to_world` uses — the
            // principal point is offset by inset/2 when a panel reserves viewport
            // edge. Omitting the inset terms here left a residual `inset/2 / ppw`
            // that changes every zoom step (ppw changes with zoom) → the centre
            // drifts horizontally on each wheel tick while a side panel is open
            // (the "zoom pans sideways at the speed of light" bug).
            self.center = WorldPoint::new(
                focus_world_before.x
                    - (focus_px.0 - viewport_px.0 * 0.5 + self.viewport_inset_right_px * 0.5) / ppw,
                focus_world_before.y
                    - (focus_px.1 - viewport_px.1 * 0.5 + self.viewport_inset_px * 0.5) / ppw,
            )
            .to_lat_lng();
        } else {
            let focus_world_after = self.pixel_to_world(focus_px, viewport_px);
            let centre = self.center.to_world();
            let dx = focus_world_after.x - focus_world_before.x;
            let dy = focus_world_after.y - focus_world_before.y;
            self.center = WorldPoint::new(centre.x - dx, centre.y - dy).to_lat_lng();
        }
        self
    }
}

/// A smooth camera transition. Driven by the host calling `Scene::tick`
/// (which calls `CameraAnimation::sample`) each frame.
///
/// The easing curve is `smoothstep` — gentle on both ends, fast in the
/// middle. For the `fly_to`-style "zoom out then in" arc, callers can
/// stack two animations: an outward zoom and then an inward one.
#[derive(Debug, Clone, Copy)]
pub struct CameraAnimation {
    start: Camera,
    target: Camera,
    started_at: Instant,
    duration: Duration,
}

impl CameraAnimation {
    pub fn new(start: Camera, target: Camera, duration: Duration) -> Self {
        Self {
            start,
            target,
            started_at: Instant::now(),
            duration,
        }
    }

    /// Construct with an explicit start time — used by tests so they can
    /// pin behaviour without sleeping.
    pub fn new_at(start: Camera, target: Camera, started_at: Instant, duration: Duration) -> Self {
        Self {
            start,
            target,
            started_at,
            duration,
        }
    }

    pub fn target(&self) -> Camera {
        self.target
    }

    /// Linear `t` ∈ [0, 1] for the animation at `now`. Returns 1.0 once the
    /// animation has elapsed (callers should drop it after that).
    pub fn linear_t(&self, now: Instant) -> f64 {
        if self.duration.is_zero() {
            return 1.0;
        }
        let elapsed = now.saturating_duration_since(self.started_at);
        (elapsed.as_secs_f64() / self.duration.as_secs_f64()).clamp(0.0, 1.0)
    }

    /// Eased `t` — smoothstep. The actual interpolation parameter passed
    /// to `Camera::lerp`.
    pub fn eased_t(&self, now: Instant) -> f64 {
        let t = self.linear_t(now);
        t * t * (3.0 - 2.0 * t)
    }

    pub fn sample(&self, now: Instant) -> Camera {
        self.start.lerp(self.target, self.eased_t(now))
    }

    pub fn is_finished(&self, now: Instant) -> bool {
        self.linear_t(now) >= 1.0
    }
}

/// Momentum (inertial fling) after a drag is released with velocity. The
/// map keeps gliding and decelerates, the way every touch map feels.
///
/// The velocity decays exponentially with time constant `tau`:
/// `v(t) = v0 · e^(−t/τ)`, so the pixel displacement from the release point
/// is `d(t) = v0 · τ · (1 − e^(−t/τ))` — frame-rate independent (sampling at
/// any cadence lands on the same curve), with a finite total throw of
/// `v0 · τ`. Sampling pans `start` by `d(t)`; the fling ends once the speed
/// drops below a small threshold.
#[derive(Debug, Clone, Copy)]
pub struct FlingAnimation {
    start: Camera,
    /// Release velocity in screen px/s (same sign convention as
    /// `pan_by_pixels`' drag delta).
    velocity_px: (f64, f64),
    started_at: Instant,
    /// Decay time constant in seconds — larger = longer glide.
    tau: f64,
}

/// Below this speed (px/s) the glide is imperceptible; stop the fling.
const FLING_STOP_SPEED: f64 = 4.0;

impl FlingAnimation {
    /// A fling from `start` released at `velocity_px` (screen px/s). The
    /// default `tau` (0.32 s) matches the gentle deceleration touch maps use.
    pub fn new(start: Camera, velocity_px: (f64, f64)) -> Self {
        Self::new_at(start, velocity_px, Instant::now(), 0.32)
    }

    /// Construct with an explicit start time + `tau` — used by tests so they
    /// can pin behaviour without sleeping.
    pub fn new_at(start: Camera, velocity_px: (f64, f64), started_at: Instant, tau: f64) -> Self {
        Self { start, velocity_px, started_at, tau }
    }

    fn elapsed(&self, now: Instant) -> f64 {
        now.saturating_duration_since(self.started_at).as_secs_f64()
    }

    /// Pixel displacement from the release point at `now`.
    fn displacement(&self, now: Instant) -> (f64, f64) {
        let decay = 1.0 - (-self.elapsed(now) / self.tau).exp();
        (
            self.velocity_px.0 * self.tau * decay,
            self.velocity_px.1 * self.tau * decay,
        )
    }

    /// Speed (px/s) remaining at `now`.
    fn speed(&self, now: Instant) -> f64 {
        let v = (-self.elapsed(now) / self.tau).exp();
        self.velocity_px.0.hypot(self.velocity_px.1) * v
    }

    pub fn sample(&self, now: Instant) -> Camera {
        let (dx, dy) = self.displacement(now);
        let mut c = self.start;
        c.pan_by_pixels(dx, dy);
        c
    }

    pub fn is_finished(&self, now: Instant) -> bool {
        self.speed(now) < FLING_STOP_SPEED
    }
}

/// Momentum on *zoom* after a pinch is released with zoom velocity — the
/// map keeps zooming and eases to a stop, about the pinch centroid.
///
/// Same exponential decay as the pan fling, in zoom-level units: the zoom
/// glides by `v0 · τ · (1 − e^(−t/τ))` levels while keeping `focus_px` over
/// the same world point (via [`Camera::zoomed_around`], so zoom stays
/// clamped). Ends once the zoom speed drops below a small threshold.
#[derive(Debug, Clone, Copy)]
pub struct ZoomFlingAnimation {
    start: Camera,
    /// Release zoom velocity in zoom-levels/second (positive = zooming in).
    zoom_velocity: f64,
    focus_px: (f64, f64),
    viewport_px: (f64, f64),
    started_at: Instant,
    tau: f64,
}

/// Below this zoom speed (levels/s) the glide is imperceptible; stop.
const ZOOM_FLING_STOP_SPEED: f64 = 0.05;

impl ZoomFlingAnimation {
    /// A zoom fling from `start` released at `zoom_velocity` (levels/s) about
    /// `focus_px`. Default `tau` (0.25 s) gives a crisp settle.
    pub fn new(
        start: Camera,
        zoom_velocity: f64,
        focus_px: (f64, f64),
        viewport_px: (f64, f64),
    ) -> Self {
        Self::new_at(start, zoom_velocity, focus_px, viewport_px, Instant::now(), 0.25)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_at(
        start: Camera,
        zoom_velocity: f64,
        focus_px: (f64, f64),
        viewport_px: (f64, f64),
        started_at: Instant,
        tau: f64,
    ) -> Self {
        Self { start, zoom_velocity, focus_px, viewport_px, started_at, tau }
    }

    fn elapsed(&self, now: Instant) -> f64 {
        now.saturating_duration_since(self.started_at).as_secs_f64()
    }

    /// Accumulated zoom-level change from the release point at `now`.
    fn delta_levels(&self, now: Instant) -> f64 {
        let decay = 1.0 - (-self.elapsed(now) / self.tau).exp();
        self.zoom_velocity * self.tau * decay
    }

    fn speed(&self, now: Instant) -> f64 {
        self.zoom_velocity.abs() * (-self.elapsed(now) / self.tau).exp()
    }

    pub fn sample(&self, now: Instant) -> Camera {
        // A delta of `d` levels is a multiplicative factor of 2^d; zooming
        // about the fixed focus keeps the pinch centroid stable.
        let factor = 2f64.powf(self.delta_levels(now));
        self.start.zoomed_around(factor, self.focus_px, self.viewport_px)
    }

    pub fn is_finished(&self, now: Instant) -> bool {
        self.speed(now) < ZOOM_FLING_STOP_SPEED
    }
}

#[cfg(test)]
mod tests {
    //! Value boundary: developers translate platform input events into camera
    //! intent and need (a) pan to move the map by the requested pixel amount
    //! and (b) zoom-around-focus to keep the focus pixel anchored to the same
    //! world point. Without these contracts, every host re-derives the math
    //! and gets it wrong.

    use super::*;

    fn matrix_is_finite(m: &[[f32; 4]; 4]) -> bool {
        m.iter().flatten().all(|v| v.is_finite())
    }

    #[test]
    fn finite_f64_parses_and_coerces() {
        assert!(FiniteF64::new(1.5).is_some());
        assert!(FiniteF64::new(f64::NAN).is_none());
        assert!(FiniteF64::new(f64::INFINITY).is_none());
        assert_eq!(FiniteF64::or(2.0, 9.0), 2.0);
        assert_eq!(FiniteF64::or(f64::NAN, 9.0), 9.0);
        assert_eq!(FiniteF64::or(f64::NEG_INFINITY, 9.0), 9.0);
    }

    #[test]
    fn sanitized_coerces_every_nonfinite_field_and_clamps_domains() {
        // A fully hostile pose: NaN centre, Inf zoom, NaN pitch, Inf bearing.
        let bad = Camera {
            center: LatLng::new(f64::NAN, f64::INFINITY),
            zoom: f64::INFINITY,
            pitch_deg: f64::NAN,
            bearing_deg: f64::NEG_INFINITY,
            viewport_inset_px: f64::NAN,
            viewport_inset_right_px: f64::INFINITY,
            zoom_bounds: ZoomBounds::DEFAULT,
        };
        let c = bad.sanitized();
        assert!(c.center.lat.is_finite() && c.center.lng.is_finite());
        assert!(c.zoom.is_finite() && (MIN_ZOOM..=MAX_ZOOM).contains(&c.zoom));
        assert!(c.pitch_deg.is_finite() && (0.0..=MAX_PITCH_DEG).contains(&c.pitch_deg));
        assert!(c.bearing_deg.is_finite() && (0.0..360.0).contains(&c.bearing_deg));
        assert!(c.viewport_inset_px.is_finite() && c.viewport_inset_px >= 0.0);
        // And the resulting matrix is finite — the whole point.
        assert!(matrix_is_finite(&c.view_projection_matrix((1080, 2400))));

        // A good pose round-trips unchanged (within clamping no-ops).
        let good = Camera::new(LatLng::new(67.25, 15.30), 13.0)
            .with_pitch(45.0)
            .with_bearing(31.0);
        let s = good.sanitized();
        assert!((s.zoom - 13.0).abs() < 1e-9);
        assert!((s.pitch_deg - 45.0).abs() < 1e-9);
        assert!((s.bearing_deg - 31.0).abs() < 1e-9);
    }

    #[test]
    fn fuzz_any_bit_pattern_pose_sanitizes_to_a_finite_matrix() {
        // The capstone: for ANY f64 a host could hand us — including every
        // NaN/Inf/subnormal/huge bit pattern — `sanitized()` followed by the
        // projection must never panic and must produce a finite matrix (the
        // value the GPU finite gate then waves through). Deterministic LCG so
        // it's reproducible; `f64::from_bits` walks the whole float space,
        // which a hand-picked value list can't.
        let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            state
        };
        for _ in 0..50_000 {
            let cam = Camera {
                center: LatLng::new(f64::from_bits(next()), f64::from_bits(next())),
                zoom: f64::from_bits(next()),
                pitch_deg: f64::from_bits(next()),
                bearing_deg: f64::from_bits(next()),
                viewport_inset_px: f64::from_bits(next()),
                viewport_inset_right_px: f64::from_bits(next()),
                zoom_bounds: ZoomBounds::DEFAULT,
            }
            .sanitized();

            // Every field came out finite + in-domain.
            assert!(cam.center.lat.is_finite() && cam.center.lng.is_finite());
            assert!((MIN_ZOOM..=MAX_ZOOM).contains(&cam.zoom));
            assert!((0.0..=MAX_PITCH_DEG).contains(&cam.pitch_deg));
            assert!((0.0..360.0).contains(&cam.bearing_deg));

            // And both projection matrices are finite at a few viewports
            // (incl. the degenerate 1×1).
            let origin = cam.center.to_world();
            for vp in [(1u32, 1u32), (1080, 2400), (3840, 2160)] {
                assert!(
                    matrix_is_finite(&cam.view_projection_matrix(vp)),
                    "non-finite VP for sanitized {cam:?} at {vp:?}"
                );
                assert!(
                    matrix_is_finite(&cam.view_projection_matrix_rtc(origin, vp)),
                    "non-finite RTC VP for sanitized {cam:?} at {vp:?}"
                );
            }
        }
    }

    #[test]
    fn pitch_sweeps_the_full_allowed_range_with_finite_matrices() {
        // The crash hunt was about steep-pitch NaN feeding the GPU. Walk pitch
        // across EVERY allowed level in 0.5° steps (at a high latitude + a
        // non-trivial bearing, the worst case for the projection) and assert
        // both the plain and RTC view-projection matrices stay finite — i.e.
        // no level the user can reach by tilting produces a NaN matrix.
        let center = LatLng::new(67.25, 15.30); // Sjunkhatten
        let origin = center.to_world();
        let viewport = (1080u32, 2400u32); // tall phone
        let mut deg = 0.0;
        while deg <= MAX_PITCH_DEG {
            let cam = Camera::new(center, 13.0)
                .with_pitch(deg)
                .with_bearing(31.0);
            assert!(
                (cam.pitch_deg - deg).abs() < 1e-9,
                "pitch within range should be honoured: {deg}",
            );
            let vp = cam.view_projection_matrix(viewport);
            assert!(matrix_is_finite(&vp), "non-finite VP at pitch {deg}");
            let vp_rtc = cam.view_projection_matrix_rtc(origin, viewport);
            assert!(matrix_is_finite(&vp_rtc), "non-finite RTC VP at pitch {deg}");
            deg += 0.5;
        }
    }

    #[test]
    fn pitch_gesture_clamps_to_the_allowed_maximum() {
        // A fast two-finger tilt (`pitch_by`) past the limit must clamp to
        // MAX_PITCH_DEG, never tilt to the horizon where the projection
        // degenerates.
        let mut cam = Camera::new(LatLng::new(67.25, 15.30), 13.0);
        cam.pitch_by(1000.0);
        assert!((cam.pitch_deg - MAX_PITCH_DEG).abs() < 1e-9);
    }

    #[test]
    fn matrix_clamps_pitch_so_an_out_of_range_pose_stays_finite() {
        // `with_pitch` stores the raw value (the gesture/host clamps are the
        // gatekeepers), but the projection itself clamps pitch into
        // `[0, MAX_PITCH_DEG]` at use — so even a bogus 120° pose can't feed a
        // degenerate, near-horizon matrix to the GPU.
        let built = Camera::new(LatLng::new(67.25, 15.30), 13.0).with_pitch(120.0);
        let vp = built.view_projection_matrix((1080, 2400));
        assert!(matrix_is_finite(&vp));
        // It matches the matrix at exactly the cap — i.e. the excess is clamped.
        let capped = Camera::new(LatLng::new(67.25, 15.30), 13.0).with_pitch(MAX_PITCH_DEG);
        assert_eq!(vp, capped.view_projection_matrix((1080, 2400)));
    }

    #[test]
    fn zoom_bounds_from_sources_is_the_union_of_ranges() {
        // A basemap serving z4..20 under an overlay serving z0..14: the lock
        // spans the union, pulling back to the overlay's shallowest (z0) and
        // diving to the basemap's deepest served level plus the overzoom
        // budget (20 + OVERZOOM_LEVELS) — past z20 the deepest tiles upsample.
        let b = ZoomBounds::from_sources([(4, 20), (0, 14)]);
        assert_eq!((b.min, b.max), (0.0, 20.0 + OVERZOOM_LEVELS));
    }

    #[test]
    fn zoom_bounds_from_sources_falls_back_to_default_when_empty() {
        // No sources → nothing to clamp to → the renderer-wide default range.
        let b = ZoomBounds::from_sources([]);
        assert_eq!(b, ZoomBounds::DEFAULT);
    }

    #[test]
    fn zoom_bounds_from_sources_pins_a_single_range() {
        // One Kartverket topograatone raster (z4..18): floor is the shallowest
        // served level (z4); ceiling is z18 plus the overzoom budget, where the
        // WMTS pyramid ends and its deepest tiles upsample.
        let b = ZoomBounds::from_sources([(4, 18)]);
        assert_eq!((b.min, b.max), (4.0, 18.0 + OVERZOOM_LEVELS));
    }

    #[test]
    fn zoom_lock_override_wins_over_sources_until_cleared() {
        let mut lock = ZoomLock::new(ZoomBounds::DEFAULT);
        // Tracking sources: resolves to the source union.
        let tracked = lock.resolve([(4, 18)]);
        assert_eq!((tracked.min, tracked.max), (4.0, 18.0 + OVERZOOM_LEVELS));

        // Host override pins an explicit range regardless of the sources.
        lock.set_manual(Some(ZoomBounds::new(8.0, 12.0)));
        let pinned = lock.resolve([(4, 18)]);
        assert_eq!((pinned.min, pinned.max), (8.0, 12.0));
        assert_eq!(lock.active(), pinned);

        // Clearing the override resumes source tracking.
        lock.set_manual(None);
        let again = lock.resolve([(4, 18)]);
        assert_eq!((again.min, again.max), (4.0, 18.0 + OVERZOOM_LEVELS));
    }

    fn assert_world_close(a: WorldPoint, b: WorldPoint, eps: f64) {
        assert!(
            (a.x - b.x).abs() < eps && (a.y - b.y).abs() < eps,
            "{:?} vs {:?}",
            a,
            b,
        );
    }

    #[test]
    fn fling_decelerates_and_converges_to_a_finite_throw() {
        // A horizontal flick: displacement grows but each interval moves
        // less (deceleration), converging on the finite total throw v0·τ.
        let start = Camera::new(LatLng::new(0.0, 0.0), 4.0);
        let t0 = Instant::now();
        let tau = 0.3;
        let fling = FlingAnimation::new_at(start, (1000.0, 0.0), t0, tau);

        let cam_at = |dt: f64| fling.sample(t0 + Duration::from_secs_f64(dt));
        let x = |c: Camera| c.center.to_world().x;
        let (x0, x1, x2) = (x(cam_at(0.0)), x(cam_at(0.1)), x(cam_at(0.2)));
        // Moves in the pan direction, and the first 100 ms covers more world
        // than the second 100 ms (it's slowing down).
        assert!(x1 < x0, "fling moves the camera");
        assert!((x0 - x1) > (x1 - x2), "fling decelerates");

        // Total throw at t→∞ is v0·τ pixels; at zoom 4 that's px/(256·2^4) world.
        let total_px = 1000.0 * tau;
        let expected_world = x0 - total_px / (256.0 * 16.0);
        let far = x(cam_at(5.0));
        assert!((far - expected_world).abs() < 1e-4, "{far} vs {expected_world}");
    }

    #[test]
    fn fling_finishes_once_it_slows_below_threshold_and_zero_is_inert() {
        let start = Camera::new(LatLng::new(0.0, 0.0), 4.0);
        let t0 = Instant::now();
        let fling = FlingAnimation::new_at(start, (1200.0, -800.0), t0, 0.3);
        assert!(!fling.is_finished(t0), "a fresh flick is animating");
        assert!(fling.is_finished(t0 + Duration::from_secs(3)), "glide ends");

        // A release with no velocity neither moves nor animates.
        let dead = FlingAnimation::new_at(start, (0.0, 0.0), t0, 0.3);
        assert!(dead.is_finished(t0));
        let s = dead.sample(t0 + Duration::from_secs_f64(0.1));
        assert!((s.center.to_world().x - start.center.to_world().x).abs() < 1e-12);
    }

    #[test]
    fn zoom_fling_glides_zoom_keeping_the_pinch_focus_fixed() {
        let viewport = (1024.0, 768.0);
        let focus = (300.0, 250.0);
        let start = Camera::new(LatLng::new(60.39, 5.32), 12.0);
        let t0 = Instant::now();
        let tau = 0.25;
        let fling = ZoomFlingAnimation::new_at(start, 4.0, focus, viewport, t0, tau);

        // The focus world point stays under the focus pixel throughout the
        // glide (that's what makes pinch-to-zoom feel anchored).
        let focus_world = start.pixel_to_world(focus, viewport);
        let zoom_at = |dt: f64| fling.sample(t0 + Duration::from_secs_f64(dt));
        for dt in [0.05, 0.15, 0.4] {
            let cam = zoom_at(dt);
            assert_world_close(cam.pixel_to_world(focus, viewport), focus_world, 1e-9);
        }
        // Zoom increases (zooming in) and decelerates: first 100 ms gains
        // more levels than the second.
        let (z0, z1, z2) = (zoom_at(0.0).zoom, zoom_at(0.1).zoom, zoom_at(0.2).zoom);
        assert!(z1 > z0 && z2 > z1, "zoom climbs");
        assert!((z1 - z0) > (z2 - z1), "zoom fling decelerates");
        // Total gain converges to v0*tau levels.
        let far = zoom_at(5.0).zoom;
        assert!((far - (z0 + 4.0 * tau)).abs() < 1e-4, "{far}");

        assert!(!fling.is_finished(t0), "fresh pinch is animating");
        assert!(fling.is_finished(t0 + Duration::from_secs(2)), "settles");
    }

    #[test]
    fn pan_moves_centre_by_expected_world_delta() {
        // At zoom 0 the world is 256 px wide. Panning by 64 px should move
        // the camera by 0.25 world units in the opposite direction.
        let mut cam = Camera::new(LatLng::new(0.0, 0.0), 0.0);
        cam.pan_by_pixels(64.0, 0.0);
        let new_world = cam.center.to_world();
        // Center was at world.x = 0.5 (lng=0). 64/256 = 0.25 westward.
        assert!((new_world.x - 0.25).abs() < 1e-9, "{}", new_world.x);
    }

    #[test]
    fn pan_at_higher_zoom_covers_proportionally_less_world() {
        // At zoom 1 the world is 512 px wide. Same 64 px = 0.125 world units.
        let mut cam = Camera::new(LatLng::new(0.0, 0.0), 1.0);
        cam.pan_by_pixels(64.0, 0.0);
        let new_world = cam.center.to_world();
        assert!(
            (new_world.x - (0.5 - 0.125)).abs() < 1e-9,
            "{}",
            new_world.x
        );
    }

    #[test]
    fn pan_round_trips_through_lat_lng_for_realistic_camera() {
        // Bergen-ish. A pan by (10, -5) should net out near-zero error after
        // round-tripping through lat/lng inside pan_by_pixels.
        let mut cam = Camera::new(LatLng::new(60.39, 5.32), 11.0);
        let before = cam.center.to_world();
        cam.pan_by_pixels(10.0, -5.0);
        let after = cam.center.to_world();
        let ppw = cam.pixels_per_world_unit();
        let expected = WorldPoint::new(before.x - 10.0 / ppw, before.y + 5.0 / ppw);
        assert_world_close(after, expected, 1e-9);
    }

    #[test]
    fn zoom_around_focus_keeps_focus_world_point_invariant() {
        // The defining property of zoom-around-focus: whatever world point is
        // under `focus_px` before, the same world point is under `focus_px`
        // after. Anything else feels broken to the user.
        let viewport = (1024.0, 768.0);
        let focus = (300.0, 200.0);
        let mut cam = Camera::new(LatLng::new(60.39, 5.32), 11.0);
        let world_before = cam.pixel_to_world(focus, viewport);
        cam.zoom_around(2.0, focus, viewport); // zoom in by 2x
        let world_after = cam.pixel_to_world(focus, viewport);
        assert_world_close(world_before, world_after, 1e-9);
    }

    #[test]
    fn zoom_around_centre_only_changes_zoom() {
        // Zooming at the centre of the viewport must not shift the camera.
        let viewport = (1024.0, 768.0);
        let centre_px = (viewport.0 * 0.5, viewport.1 * 0.5);
        let mut cam = Camera::new(LatLng::new(60.39, 5.32), 11.0);
        let before_world = cam.center.to_world();
        cam.zoom_around(2.0, centre_px, viewport);
        let after_world = cam.center.to_world();
        assert_world_close(before_world, after_world, 1e-12);
        assert!((cam.zoom - 12.0).abs() < 1e-12);
    }

    #[test]
    fn rotate_wraps_and_pitch_clamps() {
        let mut cam = Camera::new(LatLng::new(0.0, 0.0), 10.0);
        // Bearing wraps across the 0/360 seam in both directions.
        cam.rotate_by(350.0);
        cam.rotate_by(20.0); // 370 → 10
        assert!((cam.bearing_deg - 10.0).abs() < 1e-9, "{}", cam.bearing_deg);
        cam.rotate_by(-30.0); // 10 → -20 → 340
        assert!((cam.bearing_deg - 340.0).abs() < 1e-9, "{}", cam.bearing_deg);

        // Pitch clamps to [0, MAX_PITCH_DEG] and never tilts past the limit.
        cam.pitch_by(80.0);
        assert!((cam.pitch_deg - MAX_PITCH_DEG).abs() < 1e-9, "{}", cam.pitch_deg);
        cam.pitch_by(-200.0);
        assert!(cam.pitch_deg.abs() < 1e-9, "{}", cam.pitch_deg);
    }

    #[test]
    fn rotate_around_keeps_the_focus_world_point_anchored() {
        // Two-finger rotate about an off-centre pixel must keep that pixel
        // over the same place — the gesture pivots on the centroid.
        let viewport = (1024.0, 768.0);
        let focus = (760.0, 240.0);
        let cam = Camera::new(LatLng::new(60.39, 5.32), 12.0);
        let focus_world = cam.pixel_to_world(focus, viewport);
        let rotated = cam.rotated_around(37.0, focus, viewport);
        assert!((rotated.bearing_deg - 37.0).abs() < 1e-9);
        assert_world_close(rotated.pixel_to_world(focus, viewport), focus_world, 1e-5);
    }

    #[test]
    fn rotate_around_screen_centre_matches_rotate_by() {
        // Pivoting on the screen centre is exactly the simple bearing spin.
        let viewport = (1024.0, 768.0);
        let centre = (viewport.0 * 0.5, viewport.1 * 0.5);
        let mut spun = Camera::new(LatLng::new(60.39, 5.32), 11.0);
        spun.rotate_by(48.0);
        let around = Camera::new(LatLng::new(60.39, 5.32), 11.0)
            .rotated_around(48.0, centre, viewport);
        assert!((spun.bearing_deg - around.bearing_deg).abs() < 1e-9);
        assert_world_close(spun.center.to_world(), around.center.to_world(), 1e-5);
    }

    #[test]
    fn orbit_around_focus_keeps_the_pivot_world_point_anchored() {
        // The 3D-mode orbit is rotate-then-tilt about the SAME pinned focus
        // — exactly what `nativeOrbitAround` does per drag step. After both
        // edits the pivot pixel must still map to the same world point: that
        // per-step anchoring is what keeps the user's location glued to its
        // screen spot while the world spins + tilts around it. (The re-pin is
        // a one-shot correction, so the residual is the perspective non-
        // linearity over a SINGLE step — tiny; it's never re-derived from a
        // drifted base because every drag frame re-pins from scratch.)
        let viewport = (1080.0, 2160.0);
        let focus = (540.0, 700.0); // off-centre: the user dot, not screen centre
        let cam = Camera::new(LatLng::new(60.39, 5.32), 13.0);
        let pivot_world = cam.pixel_to_world(focus, viewport);

        // One representative drag step: spin the bearing and tilt, both about
        // the pinned focus.
        let orbited = cam
            .rotated_around(12.0, focus, viewport)
            .pitched_around(15.0, focus, viewport);

        assert!((orbited.bearing_deg - 12.0).abs() < 1e-9);
        assert!((orbited.pitch_deg - 15.0).abs() < 1e-9);
        assert_world_close(orbited.pixel_to_world(focus, viewport), pivot_world, 1e-5);
    }

    #[test]
    fn zoom_around_rejects_nan_and_nonpositive_factors() {
        // A NaN/≤0 zoom factor must be a no-op, not poison the zoom. A NaN
        // zoom → NaN view-projection → mobile GPU driver hang.
        let viewport = (800.0, 600.0);
        let base = Camera::new(LatLng::new(67.25, 15.3), 13.0).with_pitch(70.0);
        for bad in [f64::NAN, 0.0, -1.0, f64::INFINITY * 0.0] {
            let z = base.zoomed_around(bad, (400.0, 300.0), viewport);
            assert!(z.zoom.is_finite(), "zoom finite after factor {bad}");
            assert!((z.zoom - base.zoom).abs() < 1e-9, "no-op for factor {bad}");
            assert!(z.center.lat.is_finite() && z.center.lng.is_finite());
        }
    }

    #[test]
    fn world_to_screen_z_lifts_a_point_up_the_screen_when_pitched() {
        // Anchoring markers on 3D terrain: a point raised to a positive
        // world height (toward the sky) must project HIGHER on screen
        // (smaller y) than the same point on the flat ground, once the
        // camera is tilted. At pitch 0 the two coincide (straight down).
        let viewport = (800.0, 600.0);
        let cam = Camera::new(LatLng::new(60.39, 5.32), 13.0).with_pitch(55.0);
        let centre = cam.center.to_world();
        let flat = cam.world_to_screen(centre, viewport).expect("flat projects");
        let lifted = cam
            .world_to_screen_z(centre, 5.0e-6, viewport) // ~200 m of world-space height
            .expect("lifted projects");
        assert!(
            (lifted.0 - flat.0).abs() < 1.0,
            "x should barely move for a centred point: flat={flat:?} lifted={lifted:?}"
        );
        assert!(
            lifted.1 < flat.1 - 1.0,
            "raised point must move up the screen (smaller y): flat={flat:?} lifted={lifted:?}"
        );
    }

    #[test]
    fn orbit_at_high_zoom_keeps_focus_anchored_without_jitter() {
        // Regression: the orbit re-pin unprojected through the ABSOLUTE-frame f32
        // view-projection, whose ~0.5 Mercator coords quantise to ~6e-8 world —
        // several px at z18 — so the pivot jittered each frame when zoomed in
        // (smooth zoomed out, where ppw is small). The RTC unproject (frame
        // translated to the camera centre) keeps the residual sub-pixel.
        let viewport = (1080.0, 2160.0);
        let focus = (540.0, 800.0);
        let cam = Camera::new(LatLng::new(60.39, 5.32), 18.0).with_pitch(45.0);
        let pivot = cam.pixel_to_world(focus, viewport);
        let orbited = cam.rotated_around(20.0, focus, viewport);
        // 1px at z18 ≈ 1.5e-8 world; assert well under that. The old f32 absolute
        // frame drifted ~6e-8 here (≈4 px), which read as jitter.
        assert_world_close(orbited.pixel_to_world(focus, viewport), pivot, 5e-9);
    }

    #[test]
    fn pitch_around_anchors_focus_and_clamps() {
        let viewport = (1024.0, 768.0);
        let focus = (300.0, 600.0);
        let cam = Camera::new(LatLng::new(60.39, 5.32), 13.0);
        let focus_world = cam.pixel_to_world(focus, viewport);
        let tilted = cam.pitched_around(45.0, focus, viewport);
        assert!((tilted.pitch_deg - 45.0).abs() < 1e-9);
        assert_world_close(tilted.pixel_to_world(focus, viewport), focus_world, 1e-5);
        // Over-tilt clamps at the limit.
        let maxed = tilted.pitched_around(90.0, focus, viewport);
        assert!((maxed.pitch_deg - MAX_PITCH_DEG).abs() < 1e-9);
    }

    #[test]
    fn zoom_clamps_to_the_zoom_bounds() {
        let viewport = (1024.0, 768.0);
        let focus = (200.0, 200.0);
        // Zooming in hard from near the ceiling can't exceed MAX_ZOOM.
        let mut cam = Camera::new(LatLng::new(60.39, 5.32), MAX_ZOOM - 0.5);
        cam.zoom_around(64.0, focus, viewport); // +6 levels requested
        assert!((cam.zoom - MAX_ZOOM).abs() < 1e-12, "clamped at max: {}", cam.zoom);
        // Zooming out hard from the floor can't go below MIN_ZOOM.
        let mut cam = Camera::new(LatLng::new(0.0, 0.0), MIN_ZOOM + 0.5);
        cam.zoom_around(0.25, focus, viewport); // -2 levels requested
        assert!((cam.zoom - MIN_ZOOM).abs() < 1e-12, "clamped at min: {}", cam.zoom);
    }

    #[test]
    fn custom_zoom_bounds_lock_interactive_zoom() {
        // The map-accuracy lock: a camera bounded to [4, 18] (e.g. a source
        // whose deepest tile is z18) cannot be zoomed past 18, no matter how
        // hard the user pinches — nor below 4 zooming out.
        let viewport = (1024.0, 768.0);
        let focus = (200.0, 200.0);
        let bounds = ZoomBounds::new(4.0, 18.0);
        let mut cam = Camera::new(LatLng::new(60.39, 5.32), 17.0).with_zoom_bounds(bounds);
        cam.zoom_around(64.0, focus, viewport); // +6 levels requested → would be 23
        assert!((cam.zoom - 18.0).abs() < 1e-12, "locked at source max: {}", cam.zoom);
        cam.zoom_around(2f64.powi(-20), focus, viewport); // -20 levels requested
        assert!((cam.zoom - 4.0).abs() < 1e-12, "locked at source min: {}", cam.zoom);
    }

    #[test]
    fn with_zoom_bounds_clamps_an_out_of_range_starting_zoom() {
        // Applying tighter bounds to a camera already past them pulls the
        // zoom back into range immediately — the user doesn't stay parked on
        // a blurry over-zoomed frame after the lock is installed.
        let cam = Camera::new(LatLng::new(60.39, 5.32), 22.0)
            .with_zoom_bounds(ZoomBounds::new(4.0, 16.0));
        assert!((cam.zoom - 16.0).abs() < 1e-12, "got {}", cam.zoom);
    }

    #[test]
    fn zoom_bounds_new_orders_and_clamps_to_the_absolute_envelope() {
        // Reversed inputs are tolerated, and neither end can escape the
        // absolute [MIN_ZOOM, MAX_ZOOM] the tile math can represent.
        let b = ZoomBounds::new(18.0, 4.0);
        assert_eq!((b.min, b.max), (4.0, 18.0));
        let clamped = ZoomBounds::new(-5.0, 99.0);
        assert_eq!((clamped.min, clamped.max), (MIN_ZOOM, MAX_ZOOM));
    }

    #[test]
    fn default_bounds_preserve_the_legacy_zero_to_24_clamp() {
        // A camera with no explicit bounds behaves exactly as before the
        // lock existed — the golden/offscreen path depends on this.
        let viewport = (1024.0, 768.0);
        let focus = (200.0, 200.0);
        let mut cam = Camera::new(LatLng::new(60.39, 5.32), MAX_ZOOM - 0.5);
        cam.zoom_around(64.0, focus, viewport);
        assert!((cam.zoom - MAX_ZOOM).abs() < 1e-12, "got {}", cam.zoom);
    }

    #[test]
    fn animated_zoom_target_equals_the_immediate_zoom_result() {
        // The double-tap animation must ease toward exactly where an
        // immediate focus zoom would land — same target, just over time.
        let viewport = (1024.0, 768.0);
        let focus = (700.0, 300.0);
        let start = Camera::new(LatLng::new(60.39, 5.32), 12.0);
        let mut immediate = start;
        immediate.zoom_around(2.0, focus, viewport);
        let target = start.zoomed_around(2.0, focus, viewport);
        assert_eq!(immediate, target, "pure form matches the mutating setter");
    }

    #[test]
    fn projection_at_pitch_zero_matches_legacy_ortho() {
        // The matrix path must collapse to the legacy 2D transform when
        // pitch=0 and bearing=0 so existing tile layout, scene visibility
        // math, and on-screen positions stay pixel-stable. We check that
        // a world point at the camera centre maps to NDC origin and that
        // a 1-pixel offset in screen space corresponds to the right
        // world delta through the matrix.
        let cam = Camera::new(LatLng::new(60.39, 5.32), 11.0);
        let viewport = (1024u32, 768u32);
        let m = glam::Mat4::from_cols_array_2d(&cam.view_projection_matrix(viewport));
        let centre_world = cam.center.to_world();
        let clip = m
            * glam::Vec4::new(centre_world.x as f32, centre_world.y as f32, 0.0, 1.0);
        let ndc = clip.truncate() / clip.w;
        assert!(ndc.x.abs() < 1e-4, "x ndc = {}", ndc.x);
        assert!(ndc.y.abs() < 1e-4, "y ndc = {}", ndc.y);
    }

    #[test]
    fn pixel_to_world_round_trips_through_world_to_screen() {
        // world_to_screen(pixel_to_world(p)) must round-trip to within a
        // few pixels at any viewport coordinate so pan/zoom-around-focus
        // feel locked-on. We allow 5 px slop here because matrix
        // inversion + f32 perspective projection at zoom 11 (where one
        // world unit is half a million pixels) leaks ~1–3 px of
        // precision — production map engines have the same artifact
        // and mitigate via centre-relative world coords on the GPU
        // (a TODO for our vector pipeline, which still tessellates in
        // absolute world space).
        let viewport = (1024.0, 768.0);
        let cam = Camera::new(LatLng::new(60.39, 5.32), 11.0)
            .with_pitch(45.0)
            .with_bearing(30.0);
        for &(px, py) in &[(100.0, 100.0), (512.0, 384.0), (900.0, 500.0)] {
            let world = cam.pixel_to_world((px, py), viewport);
            let back = cam.world_to_screen(world, viewport).expect("on-screen");
            assert!(
                (back.0 - px).abs() < 5.0 && (back.1 - py).abs() < 5.0,
                "({px}, {py}) → {:?} → {:?}",
                world,
                back
            );
        }
    }

    #[test]
    fn viewport_inset_lifts_the_centre_above_the_reserved_band() {
        // A bottom inset (the live sheet) reserves `inset` px at the bottom, so
        // the map centre should render in the middle of the *visible* band —
        // half the inset above the geometric centre — and projection must stay
        // invertible so overlays still land truthfully.
        let viewport = (1024.0, 768.0);
        let inset = 240.0;
        let centre = LatLng::new(60.39, 5.32);
        let plain = Camera::new(centre, 11.0);
        let inset_cam = Camera::new(centre, 11.0).with_viewport_inset(inset);

        let world = centre.to_world();
        let plain_y = plain.world_to_screen(world, viewport).expect("on-screen").1;
        let inset_y = inset_cam.world_to_screen(world, viewport).expect("on-screen").1;

        // The centre lifts up (smaller y) by ~inset/2.
        assert!(inset_y < plain_y, "{inset_y} should sit above {plain_y}");
        assert!(
            ((plain_y - inset_y) - inset / 2.0).abs() < 1.0,
            "lift {} ≈ {}",
            plain_y - inset_y,
            inset / 2.0
        );

        // Round-trip still holds with the inset applied.
        let back = inset_cam.pixel_to_world((512.0, inset_y), viewport);
        assert_world_close(back, world, 1e-3);
    }

    #[test]
    fn rtc_keeps_high_zoom_geometry_locked_to_the_f64_projection() {
        // The desync the user saw at max zoom: GPU pipelines fed absolute f32
        // world coords (~0.5) lose precision under the huge zoom-19 scale and
        // drift away from the f64 overlay projection. The RTC matrix (origin =
        // camera centre) must track `world_to_screen` (exact f64 here) to a
        // sub-pixel, while the absolute path is visibly off.
        let vp_px = (1024.0, 768.0);
        let viewport = (1024u32, 768u32);
        let cam = Camera::new(LatLng::new(60.39, 5.32), 20.0);
        let origin = cam.center.to_world();
        let ppw = cam.pixels_per_world_unit();
        // A world point ~200 px east / 150 px south of centre.
        let world = WorldPoint::new(origin.x + 200.0 / ppw, origin.y + 150.0 / ppw);
        let truth = cam.world_to_screen(world, vp_px).expect("on-screen");

        let project = |m: [[f32; 4]; 4], p: glam::Vec4| -> (f64, f64) {
            let clip = glam::Mat4::from_cols_array_2d(&m) * p;
            let ndc = clip.truncate() / clip.w;
            (((ndc.x + 1.0) * 0.5) as f64 * vp_px.0, ((1.0 - ndc.y) * 0.5) as f64 * vp_px.1)
        };
        let dist = |a: (f64, f64), b: (f64, f64)| ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();

        let abs_px = project(
            cam.view_projection_matrix(viewport),
            glam::Vec4::new(world.x as f32, world.y as f32, 0.0, 1.0),
        );
        let rtc_px = project(
            cam.view_projection_matrix_rtc(origin, viewport),
            glam::Vec4::new((world.x - origin.x) as f32, (world.y - origin.y) as f32, 0.0, 1.0),
        );
        let abs_err = dist(abs_px, truth);
        let rtc_err = dist(rtc_px, truth);
        assert!(rtc_err < 1.0, "RTC should be sub-pixel at z19 (got {rtc_err} px; abs {abs_err} px)");
        assert!(
            abs_err > 4.0,
            "absolute f32 must be visibly off at z19 ({abs_err} px) or the test proves nothing"
        );
    }

    #[test]
    fn pan_at_zero_bearing_matches_legacy_inverse_delta() {
        // Regression: bearing-aware pan must collapse to the original
        // dx/dy formula when bearing=0. Without this guarantee every
        // platform host has to special-case pre-tilt code.
        let mut cam = Camera::new(LatLng::new(0.0, 0.0), 0.0);
        cam.pan_by_pixels(64.0, 0.0);
        let new_world = cam.center.to_world();
        assert!((new_world.x - 0.25).abs() < 1e-9, "{}", new_world.x);
    }

    #[test]
    fn pan_with_90_degree_bearing_moves_camera_east() {
        // Bearing=90 means the camera is facing east → east is the
        // top of the screen. Dragging "up" on screen (negative dy)
        // should pull the camera north (-y in world), but with the
        // bearing rotation that translates to the camera moving WEST
        // (away from east) — i.e. positive y_world. Verify the sign.
        // What we actually assert: dragging RIGHT (+dx) at bearing=90
        // moves the camera SOUTH (+y in world coords) so the map
        // scrolls north relative to the screen.
        let mut cam = Camera::new(LatLng::new(0.0, 0.0), 0.0).with_bearing(90.0);
        let before = cam.center.to_world();
        cam.pan_by_pixels(64.0, 0.0);
        let after = cam.center.to_world();
        // At bearing 90 a +dx pan rotates to a +dy world delta on the
        // *map*, so camera y *decreases* by 64/256 = 0.25 (camera moves
        // northward = -y).
        assert!((after.x - before.x).abs() < 1e-9, "{}", after.x - before.x);
        assert!(
            (after.y - (before.y - 0.25)).abs() < 1e-9,
            "{}",
            after.y - before.y
        );
    }

    #[test]
    fn zoom_factor_zero_or_negative_is_a_noop() {
        // Defensive boundary: a zero or negative factor should not produce
        // NaN / infinity in the camera state. We require a no-op.
        let viewport = (1024.0, 768.0);
        let mut cam = Camera::new(LatLng::new(60.39, 5.32), 11.0);
        let before = cam;
        cam.zoom_around(0.0, (100.0, 100.0), viewport);
        assert_eq!(cam, before);
        cam.zoom_around(-1.0, (100.0, 100.0), viewport);
        assert_eq!(cam, before);
    }

    // ---- animation contract --------------------------------------------

    #[test]
    fn lerp_at_zero_is_start_at_one_is_target() {
        let a = Camera::new(LatLng::new(60.0, 5.0), 10.0);
        let b = Camera::new(LatLng::new(70.0, 18.0), 12.5);
        let r0 = a.lerp(b, 0.0);
        let r1 = a.lerp(b, 1.0);
        // Round-tripping through Mercator introduces ~1e-13 error.
        assert!((r0.center.lat - a.center.lat).abs() < 1e-9);
        assert!((r0.center.lng - a.center.lng).abs() < 1e-9);
        assert!((r0.zoom - a.zoom).abs() < 1e-12);
        assert!((r1.center.lat - b.center.lat).abs() < 1e-9);
        assert!((r1.center.lng - b.center.lng).abs() < 1e-9);
        assert!((r1.zoom - b.zoom).abs() < 1e-12);
    }

    #[test]
    fn animation_finishes_exactly_at_the_target() {
        // Sampling at start + duration produces the target camera (within
        // float precision). This is the core contract: developers schedule
        // `ease_to(target)` and expect to land on `target` cleanly.
        let start = Camera::new(LatLng::new(60.0, 5.0), 10.0);
        let target = Camera::new(LatLng::new(63.4, 10.3), 14.0);
        let t0 = Instant::now();
        let anim = CameraAnimation::new_at(start, target, t0, Duration::from_millis(300));
        let at_end = t0 + Duration::from_millis(300);
        assert!(anim.is_finished(at_end));
        let sample = anim.sample(at_end);
        assert!((sample.center.lat - target.center.lat).abs() < 1e-9);
        assert!((sample.center.lng - target.center.lng).abs() < 1e-9);
        assert!((sample.zoom - target.zoom).abs() < 1e-12);
    }

    #[test]
    fn animation_midpoint_is_between_start_and_target() {
        // Mid-duration sample should be strictly between start and target
        // on each axis. Smoothstep is monotone so this holds at any
        // intermediate time.
        let start = Camera::new(LatLng::new(60.0, 5.0), 10.0);
        let target = Camera::new(LatLng::new(63.4, 10.3), 14.0);
        let t0 = Instant::now();
        let anim = CameraAnimation::new_at(start, target, t0, Duration::from_millis(400));
        let mid = anim.sample(t0 + Duration::from_millis(200));
        assert!(mid.zoom > start.zoom && mid.zoom < target.zoom);
        assert!(mid.center.lat > start.center.lat && mid.center.lat < target.center.lat);
        assert!(mid.center.lng > start.center.lng && mid.center.lng < target.center.lng);
        assert!(!anim.is_finished(t0 + Duration::from_millis(200)));
    }

    #[test]
    fn animation_easing_starts_and_ends_with_zero_velocity() {
        // smoothstep produces tangent slope 0 at both ends. We can't measure
        // velocity directly, but t-just-after-start should be very close to
        // start (not linearly close), and t-just-before-end close to end.
        let start = Camera::new(LatLng::new(0.0, 0.0), 10.0);
        let target = Camera::new(LatLng::new(0.0, 0.0), 20.0);
        let t0 = Instant::now();
        let dur = Duration::from_secs(1);
        let anim = CameraAnimation::new_at(start, target, t0, dur);
        let just_after = anim.sample(t0 + Duration::from_millis(50));
        // Linear easing would give zoom = 10.5; smoothstep at t=0.05 gives
        // ~0.0073 → zoom ≈ 10.073. Much closer to start than linear.
        assert!(just_after.zoom < 10.2, "got {}", just_after.zoom);
    }

    #[test]
    fn zero_duration_animation_lands_on_target_immediately() {
        // Edge case: `ease_to` with duration=0 must not divide-by-zero.
        let start = Camera::new(LatLng::new(0.0, 0.0), 10.0);
        let target = Camera::new(LatLng::new(45.0, 45.0), 14.0);
        let t0 = Instant::now();
        let anim = CameraAnimation::new_at(start, target, t0, Duration::ZERO);
        assert!(anim.is_finished(t0));
        let sample = anim.sample(t0);
        assert!((sample.zoom - target.zoom).abs() < 1e-12);
    }

    // --- Phase 2: horizon far-plane + Earth-curvature droop ---

    const EARTH_CIRCUMFERENCE_M: f64 = 40_075_017.0;
    const EARTH_RADIUS_M: f64 = EARTH_CIRCUMFERENCE_M / (2.0 * std::f64::consts::PI);

    #[test]
    fn curvature_drop_matches_d_squared_over_2r() {
        // The shader lowers world_z by `coeff · s_world²`. Converting that drop
        // back to metres (through the same anisotropic Mercator scales the engine
        // uses for elevation) must reproduce the textbook `s²/2R` — proving the
        // π·cos³φ coefficient folds the radius + both scales correctly.
        let lat = 67.23_f64.to_radians();
        let coslat = lat.cos();
        let coeff = earth_curvature_coeff(coslat as f32) as f64;
        for s_m in [5_000.0_f64, 30_000.0, 100_000.0] {
            // ground metres → horizontal world units (1 world = C·cosφ m E-W).
            let s_world = s_m / (EARTH_CIRCUMFERENCE_M * coslat);
            let drop_world = coeff * s_world * s_world;
            // vertical world units → metres (elevation uses cosφ/C).
            let drop_m = drop_world * (EARTH_CIRCUMFERENCE_M / coslat);
            let expected = s_m * s_m / (2.0 * EARTH_RADIUS_M);
            assert!(
                (drop_m - expected).abs() < expected * 0.02,
                "curvature drop at {s_m} m: got {drop_m:.1} m, expected {expected:.1} m"
            );
        }
    }

    #[test]
    fn ground_horizon_matches_spherical_formula() {
        // ground_horizon_world must equal the spherical horizon distance
        // sqrt(2·R·h) for the camera's eye height, expressed in horizontal world.
        let lat = 67.23_f64.to_radians();
        let coslat = lat.cos();
        let cam = Camera::new(LatLng::new(67.23, 15.30), 14.0).with_pitch(80.0);
        let vp = (1080u32, 1600u32);
        let altitude = cam.altitude_world(vp) as f64;
        let pitch = 80.0_f64.to_radians();
        // Eye height in metres (vertical world → metres via C/cosφ).
        let eye_h_m = altitude * pitch.cos() * (EARTH_CIRCUMFERENCE_M / coslat);
        let expected_world = (2.0 * EARTH_RADIUS_M * eye_h_m).sqrt() / (EARTH_CIRCUMFERENCE_M * coslat);
        let got = ground_horizon_world(altitude as f32, pitch as f32, coslat as f32) as f64;
        assert!(
            (got - expected_world).abs() < expected_world * 0.02,
            "horizon: got {got:.5} world, expected {expected_world:.5} world"
        );
    }

    #[test]
    fn vp_finite_at_pitch_80_with_far_horizon() {
        // The far-plane horizon extension must keep the view-projection finite at
        // grazing pitch across the zoom range (high zoom = small altitude = the
        // largest far/near ratio).
        for zoom in [10.0_f64, 14.0, 17.0, 19.0] {
            let cam = Camera::new(LatLng::new(67.23, 15.30), zoom).with_pitch(80.0).with_bearing(33.0);
            let m = cam.view_projection_matrix_rtc(cam.center.to_world(), (1080, 1600));
            assert!(matrix_is_finite(&m), "vp must be finite at zoom {zoom}, pitch 80");
        }
    }
}
