import { create } from 'zustand';

/** The Sun slider's state: a normalized level `[0, 1]` (0 == off). The derived
 *  environment (`deriveMapEnvironment`) turns this into a sun position/time and
 *  relief lighting — the slider never touches the camera. Owned by this slice
 *  (was `on` + a free-floating `hour` scattered across `uiStore.sun` +
 *  MapScreen). */
interface SunState {
  level: number;
  setLevel: (v: number) => void;
}

export const useSunStore = create<SunState>((set) => ({
  level: 0,
  setLevel: (level) => set({ level }),
}));
