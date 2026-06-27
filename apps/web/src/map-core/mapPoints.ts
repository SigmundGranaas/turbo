import { create } from 'zustand';
import type { LatLng } from '../geo';

/** Transient on-map indicator points the kernel renders via `MapPointMarkers`:
 *  - `click` — where the user last clicked (the context-menu anchor), so a tap
 *    has a visible "you tapped here" target.
 *  - `orbit` — the terrain point the 3D camera is orbiting around, shown only
 *    while an orbit/tilt gesture is live.
 *  Both are projected terrain-aware via `project()`, so they sit ON the 3D
 *  relief, never the flat baseline. */
interface MapPointsState {
  click: LatLng | null;
  orbit: LatLng | null;
  setClick: (p: LatLng | null) => void;
  setOrbit: (p: LatLng | null) => void;
}

export const useMapPoints = create<MapPointsState>((set) => ({
  click: null,
  orbit: null,
  setClick: (click) => set({ click }),
  setOrbit: (orbit) => set({ orbit }),
}));
