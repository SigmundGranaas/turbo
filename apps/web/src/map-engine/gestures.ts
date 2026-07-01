import type { TurboMap } from 'turbomap-web';

/** Semantic callbacks the host (MapScreen) wires to app behaviour. Coordinates
 *  are CSS pixels relative to the viewport (the canvas fills it); the host
 *  multiplies by DPR before `unproject`. */
export interface GestureCallbacks {
  /** The live engine, or null before it boots. */
  getMap: () => TurboMap | null;
  /** Current 3D state (so a tilt/orbit gesture knows whether to request 3D). */
  is3d: () => boolean;
  /** Request a flip to 3D — fired when an orbit/tilt gesture begins in 2D. */
  onEnter3d: () => void;
  /** A click (mouse) / tap (touch) that wasn't a drag, double, or long-press. */
  onTap: (x: number, y: number, pointerType: string) => void;
  /** A touch long-press (~500ms held still) — the mobile "add marker" gesture. */
  onLongPress: (x: number, y: number) => void;
  /** The terrain point a 3D orbit/tilt gesture is pivoting around (the focus
   *  pixel raycast onto the relief), or `null` when the gesture ends. The host
   *  shows a pin there so there's a visible anchor sitting on the 3D geometry. */
  onOrbit?: (anchor: { lat: number; lng: number } | null) => void;
}

// Tuning — mirror the feel of Google/Apple/Mapbox.
const TAP_MOVE = 6; // px of movement that turns a tap into a drag
const ORBIT_ARM = 3; // px before a mouse orbit/tilt engages (so a bare click doesn't)
const LONG_PRESS_MS = 500;
const DOUBLE_MS = 300;
const DOUBLE_DIST = 30;
const FLING_MIN = 80; // px/s release speed below which we don't fling
const BEARING_PER_PX = 0.45; // mouse-orbit horizontal sensitivity
const PITCH_PER_PX = 0.4; // mouse-orbit vertical sensitivity
// Two-finger orbit sensitivities in 3D — match the Android app (0.3 deg/px), so
// the touch feel is identical across web and Android.
const TOUCH_BEARING_PER_PX = 0.3;
const TOUCH_PITCH_PER_PX = 0.3;

const isTouch = (t: string) => t === 'touch' || t === 'pen';

/** Attach all map gestures to `canvas`. Returns a `dispose()` that removes every
 *  listener + timer. One unified Pointer-Events state machine handles mouse +
 *  touch + pen; `wheel`/`dblclick`/`keydown` cover desktop affordances.
 *
 *  Touch model (identical to the Android app): 1 finger pans (inertial fling on
 *  release) in BOTH 2D and 3D; 2 fingers always zoom about the centroid, and the
 *  centroid's drag does the second axis — in 2D it pans, in 3D it ORBITS
 *  (horizontal → bearing, vertical → pitch) and never pans. Finger twist is
 *  ignored in both modes. Long-press adds a marker; double-tap zooms. Mouse
 *  (desktop only): drag pans, wheel/double-click zoom, right-drag or Ctrl/⌘-drag
 *  orbits + tilts. */
export function attachMapGestures(
  canvas: HTMLCanvasElement,
  dpr: number,
  cb: GestureCallbacks,
): () => void {
  const now = () => performance.now();
  const focusPx = (x: number, y: number): [number, number] => [x * dpr, y * dpr];
  // Report the ground point under a 3D pivot pixel (device px) so the host can
  // pin it on the relief. Terrain-aware raycast; falls back to the flat plane.
  const reportOrbit = (map: TurboMap, fx: number, fy: number) => {
    if (!cb.onOrbit) return;
    const g = map.unproject_ground(fx, fy);
    if (g && g.length >= 2) cb.onOrbit({ lat: g[0], lng: g[1] });
  };

  // --- shared pointer set ---
  const pointers = new Map<number, { x: number; y: number }>();
  const sortedTwo = () => {
    const ids = [...pointers.keys()].sort((a, b) => a - b);
    return [pointers.get(ids[0])!, pointers.get(ids[1])!] as const;
  };

  // --- single-pointer tap / pan / long-press ---
  let primaryId: number | null = null;
  let downX = 0;
  let downY = 0;
  let moved = false;
  let consumed = false; // a long-press / 2-finger gesture happened → no tap on release
  let longPressTimer: ReturnType<typeof setTimeout> | null = null;
  let panV: { t: number; x: number; y: number }[] = [];
  // deferred single-tap (so a double-tap can pre-empt it for zoom)
  let lastTap = { t: -1e9, x: 0, y: 0 };
  let tapTimer: ReturnType<typeof setTimeout> | null = null;

  // --- mouse orbit/tilt (right-drag or modifier-drag) ---
  let orbitId: number | null = null;
  let orbitPrevX = 0;
  let orbitPrevY = 0;
  let orbitStartX = 0;
  let orbitStartY = 0;
  let orbitArmed = false;

  // --- two-finger gesture (Android parity: 2D → pan+zoom, 3D → orbit+zoom;
  //     finger twist ignored in both modes) ---
  let twoIs3d = false; // 3D sampled once at gesture start so the grammar is stable
  let snapAx = 0; // rolling previous frame (for incremental deltas)
  let snapAy = 0;
  let snapBx = 0;
  let snapBy = 0;
  let pinchZoom = 0; // cumulative zoom levels, for zoom-fling
  let pinchV: { t: number; z: number }[] = [];

  const clearLongPress = () => {
    if (longPressTimer != null) {
      clearTimeout(longPressTimer);
      longPressTimer = null;
    }
  };
  const wantsOrbit = (e: PointerEvent) =>
    e.pointerType === 'mouse' && (e.button === 2 || e.ctrlKey || e.metaKey);

  const velFrom = (s: { t: number; x: number; y: number }[]) => {
    if (s.length < 2) return { vx: 0, vy: 0 };
    const last = s[s.length - 1];
    let i = s.length - 1;
    while (i > 0 && last.t - s[i - 1].t < 60) i--;
    const a = s[i];
    const dt = last.t - a.t;
    if (dt <= 0) return { vx: 0, vy: 0 };
    return { vx: (last.x - a.x) / dt, vy: (last.y - a.y) / dt };
  };

  const beginTwoFinger = () => {
    clearLongPress();
    cancelPendingTap();
    consumed = true; // the touch became a gesture, not a tap
    const [a, b] = sortedTwo();
    snapAx = a.x; snapAy = a.y; snapBx = b.x; snapBy = b.y;
    // Sample the mode once: like Android, a mid-gesture 2D↔3D flip can't change
    // whether the two-finger drag pans (2D) or orbits (3D).
    twoIs3d = cb.is3d();
    pinchZoom = 0;
    pinchV = [{ t: now(), z: 0 }];
  };

  const cancelPendingTap = () => {
    if (tapTimer != null) {
      clearTimeout(tapTimer);
      tapTimer = null;
    }
  };

  const onDown = (e: PointerEvent) => {
    canvas.focus?.();
    const map = cb.getMap();
    // Mouse orbit/tilt — armed only after real movement (a bare right-click
    // must NOT flip into 3D).
    if (wantsOrbit(e)) {
      orbitId = e.pointerId;
      orbitPrevX = e.clientX;
      orbitPrevY = e.clientY;
      orbitStartX = e.clientX;
      orbitStartY = e.clientY;
      orbitArmed = false;
      canvas.setPointerCapture(e.pointerId);
      e.preventDefault();
      return;
    }
    if (map) map.pan_by_pixels(0, 0); // touch-to-stop any running fling
    pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
    canvas.setPointerCapture(e.pointerId);

    if (pointers.size === 1) {
      primaryId = e.pointerId;
      downX = e.clientX;
      downY = e.clientY;
      moved = false;
      consumed = false;
      panV = [{ t: now(), x: e.clientX, y: e.clientY }];
      if (isTouch(e.pointerType)) {
        clearLongPress();
        longPressTimer = setTimeout(() => {
          longPressTimer = null;
          if (!moved && pointers.size === 1) {
            consumed = true;
            cb.onLongPress(downX, downY);
          }
        }, LONG_PRESS_MS);
      }
    } else if (pointers.size === 2) {
      beginTwoFinger();
    }
  };

  const onMove = (e: PointerEvent) => {
    const map = cb.getMap();
    if (orbitId === e.pointerId && map) {
      const dx = e.clientX - orbitPrevX;
      const dy = e.clientY - orbitPrevY;
      orbitPrevX = e.clientX;
      orbitPrevY = e.clientY;
      if (!orbitArmed) {
        if (Math.hypot(e.clientX - orbitStartX, e.clientY - orbitStartY) < ORBIT_ARM) return;
        orbitArmed = true;
        if (!cb.is3d()) cb.onEnter3d();
      }
      // drag right → rotate the map the way the drag points (grab-and-turn, like
      // Google/Apple), drag up → tilt toward horizon, pivot under cursor.
      map.orbit_around(dx * BEARING_PER_PX, -dy * PITCH_PER_PX, ...focusPx(e.clientX, e.clientY));
      reportOrbit(map, ...focusPx(e.clientX, e.clientY));
      return;
    }
    if (!map || !pointers.has(e.pointerId)) return;
    pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });

    if (pointers.size >= 2) {
      twoFingerMove(map);
      return;
    }

    // single-pointer pan
    const dxTotal = e.clientX - downX;
    const dyTotal = e.clientY - downY;
    if (!moved && Math.hypot(dxTotal, dyTotal) > TAP_MOVE) {
      moved = true;
      clearLongPress();
    }
    if (moved) {
      const prev = panV[panV.length - 1];
      map.pan_by_pixels((e.clientX - prev.x) * dpr, (e.clientY - prev.y) * dpr);
      panV.push({ t: now(), x: e.clientX, y: e.clientY });
      if (panV.length > 8) panV.shift();
    }
  };

  const twoFingerMove = (map: TurboMap) => {
    const [a, b] = sortedTwo();
    const cx = (a.x + b.x) / 2;
    const cy = (a.y + b.y) / 2;
    const dist = Math.hypot(a.x - b.x, a.y - b.y);
    const prevCx = (snapAx + snapBx) / 2;
    const prevCy = (snapAy + snapBy) / 2;
    const prevDist = Math.hypot(snapAx - snapBx, snapAy - snapBy);

    // Zoom about the moving two-finger centroid — always, in both modes.
    if (prevDist > 0 && dist > 0) {
      const factor = dist / prevDist;
      map.zoom_around(factor, ...focusPx(cx, cy));
      pinchZoom += Math.log2(factor);
      pinchV.push({ t: now(), z: pinchZoom });
      if (pinchV.length > 8) pinchV.shift();
    }

    // The centroid's translation drives the second axis (finger TWIST is ignored
    // in both modes — Android parity). 2D: the drag pans. 3D: the drag ORBITS —
    // horizontal → bearing, vertical → pitch — and never pans (orbit_around
    // re-pins the centroid pixel so the map doesn't drift).
    const dCx = cx - prevCx;
    const dCy = cy - prevCy;
    if (twoIs3d) {
      map.orbit_around(dCx * TOUCH_BEARING_PER_PX, -dCy * TOUCH_PITCH_PER_PX, ...focusPx(cx, cy));
      reportOrbit(map, ...focusPx(cx, cy));
    } else {
      map.pan_by_pixels(dCx * dpr, dCy * dpr);
    }
    snapAx = a.x; snapAy = a.y; snapBx = b.x; snapBy = b.y;
  };

  const handleTapRelease = (x: number, y: number, pointerType: string) => {
    const t = now();
    if (t - lastTap.t < DOUBLE_MS && Math.hypot(x - lastTap.x, y - lastTap.y) < DOUBLE_DIST) {
      // Second tap → double-tap/click zoom; cancel the pending single-tap.
      cancelPendingTap();
      lastTap = { t: -1e9, x: 0, y: 0 };
      cb.getMap()?.zoom_around_animated(2.0, ...focusPx(x, y), 200);
      return;
    }
    lastTap = { t, x, y };
    // Defer the single-tap so a follow-up tap can pre-empt it (→ double zoom).
    cancelPendingTap();
    tapTimer = setTimeout(() => {
      tapTimer = null;
      cb.onTap(x, y, pointerType);
    }, DOUBLE_MS);
  };

  const onUp = (e: PointerEvent) => {
    const map = cb.getMap();
    if (orbitId === e.pointerId) {
      orbitId = null;
      cb.onOrbit?.(null);
      try { canvas.releasePointerCapture(e.pointerId); } catch { /* released */ }
      return;
    }
    const wasTwo = pointers.size >= 2;
    const two = wasTwo ? sortedTwo() : null;
    pointers.delete(e.pointerId);
    try { canvas.releasePointerCapture(e.pointerId); } catch { /* released */ }
    if (!map) return;

    if (wasTwo) {
      // Pinch ended: zoom-fling from the recent zoom-rate (levels/sec).
      const last = pinchV[pinchV.length - 1];
      let i = pinchV.length - 1;
      while (i > 0 && last && last.t - pinchV[i - 1].t < 60) i--;
      const aSample = pinchV[i];
      const dt = last && aSample ? last.t - aSample.t : 0;
      const zv = dt > 0 ? ((last.z - aSample.z) / dt) * 1000 : 0;
      // Zoom-fling from the pinch velocity, in both 2D and 3D (Android parity).
      if (two && Math.abs(zv) > 0.3) {
        const cx = (two[0].x + two[1].x) / 2;
        const cy = (two[0].y + two[1].y) / 2;
        map.zoom_fling(zv, ...focusPx(cx, cy));
      }
      cb.onOrbit?.(null); // drop the orbit pin if this was a 3D orbit gesture
      // A finger remains → reset it as a fresh pan baseline (no tap, gesture
      // already happened).
      if (pointers.size === 1) {
        const [id, p] = [...pointers.entries()][0];
        primaryId = id;
        downX = p.x; downY = p.y; moved = false; consumed = true;
        panV = [{ t: now(), x: p.x, y: p.y }];
      }
      return;
    }

    clearLongPress();
    if (e.pointerId !== primaryId) return;
    primaryId = null;
    if (consumed) return; // long-press already fired
    if (moved) {
      const { vx, vy } = velFrom(panV);
      if (Math.hypot(vx, vy) * 1000 > FLING_MIN) map.fling(vx * 1000 * dpr, vy * 1000 * dpr);
      return;
    }
    // A genuine tap/click.
    handleTapRelease(e.clientX, e.clientY, e.pointerType);
  };

  const onWheel = (e: WheelEvent) => {
    e.preventDefault();
    const map = cb.getMap();
    if (!map) return;
    // Scale the zoom by the ACTUAL wheel delta (normalised across deltaMode),
    // not a fixed step — otherwise a trackpad (which fires many tiny events)
    // zooms wildly. Mouse wheels send large deltas (one notch ≈ ±100), so they
    // still feel snappy; a trackpad's small deltas give smooth fine zoom.
    // ctrl+wheel is the trackpad pinch gesture → treat as fine.
    let dy = e.deltaY;
    if (e.deltaMode === 1) dy *= 16; // lines → px
    else if (e.deltaMode === 2) dy *= 400; // pages → px
    const fine = e.ctrlKey || Math.abs(dy) < 60;
    const k = fine ? 0.0022 : 0.0055;
    // factor = 2^(zoom-level delta); clamp per-event so a jumbo delta can't leap.
    const factor = Math.pow(2, Math.max(-1.5, Math.min(1.5, (-dy * k))));
    // Instant (not animated) so continuous trackpad scroll tracks smoothly;
    // the engine's tile fade keeps it visually clean. Focus stays under cursor.
    map.zoom_around(factor, ...focusPx(e.clientX, e.clientY));
  };

  // Native double-click is the reliable desktop double — touch goes through
  // handleTapRelease (synthetic dblclick is unreliable on touch).
  const onDblClick = (e: MouseEvent) => {
    e.preventDefault();
    cancelPendingTap();
    cb.getMap()?.zoom_around_animated(2.0, ...focusPx(e.clientX, e.clientY), 200);
  };

  const onContextMenu = (e: Event) => e.preventDefault();

  const onKeyDown = (e: KeyboardEvent) => {
    const map = cb.getMap();
    if (!map) return;
    const PAN = 80 * dpr;
    const cx = (canvas.clientWidth / 2) * dpr;
    const cy = (canvas.clientHeight / 2) * dpr;
    let handled = true;
    switch (e.key) {
      case 'ArrowUp': if (e.shiftKey) map.orbit_around(0, 6, cx, cy); else map.pan_by_pixels(0, PAN); break;
      case 'ArrowDown': if (e.shiftKey) map.orbit_around(0, -6, cx, cy); else map.pan_by_pixels(0, -PAN); break;
      case 'ArrowLeft': if (e.shiftKey) map.orbit_around(-10, 0, cx, cy); else map.pan_by_pixels(PAN, 0); break;
      case 'ArrowRight': if (e.shiftKey) map.orbit_around(10, 0, cx, cy); else map.pan_by_pixels(-PAN, 0); break;
      case '+': case '=': map.zoom_around_animated(1.6, cx, cy, 200); break;
      case '-': case '_': map.zoom_around_animated(1 / 1.6, cx, cy, 200); break;
      case '0': {
        try {
          const c = JSON.parse(map.camera_json()) as { lat: number; lng: number; zoom: number };
          map.ease_to(c.lat, c.lng, c.zoom, 0, 300);
        } catch { /* ignore */ }
        break;
      }
      default: handled = false;
    }
    if (handled) e.preventDefault();
  };

  canvas.addEventListener('pointerdown', onDown);
  canvas.addEventListener('pointerup', onUp);
  canvas.addEventListener('pointercancel', onUp);
  canvas.addEventListener('pointermove', onMove);
  canvas.addEventListener('wheel', onWheel, { passive: false });
  canvas.addEventListener('dblclick', onDblClick);
  canvas.addEventListener('contextmenu', onContextMenu);
  canvas.addEventListener('keydown', onKeyDown);

  return () => {
    clearLongPress();
    cancelPendingTap();
    canvas.removeEventListener('pointerdown', onDown);
    canvas.removeEventListener('pointerup', onUp);
    canvas.removeEventListener('pointercancel', onUp);
    canvas.removeEventListener('pointermove', onMove);
    canvas.removeEventListener('wheel', onWheel);
    canvas.removeEventListener('dblclick', onDblClick);
    canvas.removeEventListener('contextmenu', onContextMenu);
    canvas.removeEventListener('keydown', onKeyDown);
  };
}
