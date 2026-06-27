import { create } from 'zustand';
import type { LatLng } from '../geo';

/** The click indicator point the kernel renders via `MapPointMarkers` — where
 *  the user last clicked (the context-menu anchor), so a tap has a visible "you
 *  tapped here" target. Projected terrain-aware via `project()`, so it sits ON
 *  the 3D relief, never the flat baseline. */
interface MapPointsState {
  click: LatLng | null;
  setClick: (p: LatLng | null) => void;
}

export const useMapPoints = create<MapPointsState>((set) => ({
  click: null,
  setClick: (click) => set({ click }),
}));
