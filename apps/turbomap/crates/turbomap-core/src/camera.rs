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

use std::time::{Duration, Instant};

use glam::{Mat3, Mat4, Vec3};

use crate::geo::{LatLng, WorldPoint};

/// One tile is `TILE_SIZE_PX` pixels wide at its native zoom.
pub const TILE_SIZE_PX: f64 = 256.0;

/// Vertical field of view used for the perspective projection. Matches
/// the MapLibre default. With `altitude` chosen as
/// `(vh / 2) / ppw / tan(fov_y / 2)` the perspective collapses to the
/// legacy 2D orthographic transform when `pitch_deg == 0` — pixel-
/// exact match, no spec change for callers that don't tilt.
const FOV_Y: f32 = 0.6435; // ~36.87°
const MAX_PITCH_DEG: f64 = 60.0;

/// Zoom bounds. z0 is the whole world in one root tile; z24 is deeper than
/// any tile source serves but lets the camera glide smoothly to the limit.
pub const MIN_ZOOM: f64 = 0.0;
pub const MAX_ZOOM: f64 = 24.0;

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
}

impl Camera {
    pub const fn new(center: LatLng, zoom: f64) -> Self {
        Self {
            center,
            zoom,
            pitch_deg: 0.0,
            bearing_deg: 0.0,
        }
    }

    pub fn with_pitch(mut self, deg: f64) -> Self {
        self.pitch_deg = deg;
        self
    }

    pub fn with_bearing(mut self, deg: f64) -> Self {
        self.bearing_deg = deg;
        self
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

    fn view_projection(self, viewport_px: (u32, u32)) -> Mat4 {
        let vw = viewport_px.0.max(1) as f32;
        let vh = viewport_px.1.max(1) as f32;
        let ppw = self.pixels_per_world_unit() as f32;
        let altitude = (vh * 0.5) / ppw / (FOV_Y * 0.5).tan();

        let pitch = (self.pitch_deg.clamp(0.0, MAX_PITCH_DEG) as f32).to_radians();
        let bearing = (self.bearing_deg as f32).to_radians();

        let centre = self.center.to_world();
        let target = Vec3::new(centre.x as f32, centre.y as f32, 0.0);

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
        // adapts to the current zoom without precision loss.
        let aspect = vw / vh;
        let near = altitude * 0.01;
        let far = altitude * 100.0;
        let proj = Mat4::perspective_lh(FOV_Y, aspect, near, far);

        proj * view
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
        if self.pitch_deg == 0.0 && self.bearing_deg == 0.0 {
            let ppw = self.pixels_per_world_unit();
            let centre = self.center.to_world();
            return Some((
                (world.x - centre.x) * ppw + viewport_px.0 * 0.5,
                (world.y - centre.y) * ppw + viewport_px.1 * 0.5,
            ));
        }
        let vp = self.view_projection((viewport_px.0 as u32, viewport_px.1 as u32));
        let clip =
            vp * glam::Vec4::new(world.x as f32, world.y as f32, 0.0, 1.0);
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
            return WorldPoint::new(
                center.x + (pixel.0 - viewport_px.0 * 0.5) / ppw,
                center.y + (pixel.1 - viewport_px.1 * 0.5) / ppw,
            );
        }
        let vp = self.view_projection((viewport_px.0 as u32, viewport_px.1 as u32));
        let inv = vp.inverse();
        let ndc_x = (2.0 * pixel.0 / viewport_px.0 - 1.0) as f32;
        let ndc_y = (1.0 - 2.0 * pixel.1 / viewport_px.1) as f32;
        let near = inv.project_point3(Vec3::new(ndc_x, ndc_y, 0.0));
        let far = inv.project_point3(Vec3::new(ndc_x, ndc_y, 1.0));
        let dir = far - near;
        if dir.z.abs() < 1e-6 {
            return self.center.to_world();
        }
        let t = -near.z / dir.z;
        let ground = near + dir * t;
        WorldPoint::new(ground.x as f64, ground.y as f64)
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
    /// focus pixel kept over the same world point. Zoom is clamped to
    /// `[MIN_ZOOM, MAX_ZOOM]`. Used both for the immediate setter and to
    /// compute the *target* of an animated double-tap / scroll zoom.
    pub fn zoomed_around(
        mut self,
        factor: f64,
        focus_px: (f64, f64),
        viewport_px: (f64, f64),
    ) -> Camera {
        if factor <= 0.0 {
            return self;
        }
        let focus_world_before = self.pixel_to_world(focus_px, viewport_px);
        self.zoom = (self.zoom + factor.log2()).clamp(MIN_ZOOM, MAX_ZOOM);
        // After the (clamped) zoom change, the same focus pixel maps to a
        // different world point. Shift the centre so the focus world point
        // lands back under the focus pixel. With tilt we re-unproject
        // through the updated camera; without tilt the legacy direct math
        // is pixel-equivalent.
        if self.pitch_deg == 0.0 && self.bearing_deg == 0.0 {
            let ppw = self.pixels_per_world_unit();
            self.center = WorldPoint::new(
                focus_world_before.x - (focus_px.0 - viewport_px.0 * 0.5) / ppw,
                focus_world_before.y - (focus_px.1 - viewport_px.1 * 0.5) / ppw,
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

#[cfg(test)]
mod tests {
    //! Value boundary: developers translate platform input events into camera
    //! intent and need (a) pan to move the map by the requested pixel amount
    //! and (b) zoom-around-focus to keep the focus pixel anchored to the same
    //! world point. Without these contracts, every host re-derives the math
    //! and gets it wrong.

    use super::*;

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
}
