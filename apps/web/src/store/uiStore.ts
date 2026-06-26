import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import type { BaseLayerId } from '../map-engine';

export type ThemeMode = 'system' | 'light' | 'dark';
export type Units = 'metric' | 'imperial';

/** Client/session UI state. Theme + units are persisted (localStorage); the rest
 *  is per-session. Mirrors the bits Android keeps in MapUiState + Settings. */
interface UiState {
  theme: ThemeMode;
  units: Units;
  baseLayer: BaseLayerId;
  threeD: boolean;
  layers: boolean;
  following: boolean;
  setTheme: (t: ThemeMode) => void;
  setUnits: (u: Units) => void;
  setBaseLayer: (b: BaseLayerId) => void;
  setThreeD: (v: boolean) => void;
  setLayers: (v: boolean) => void;
  setFollowing: (v: boolean) => void;
}

export const useUiStore = create<UiState>()(
  persist(
    (set) => ({
      theme: 'system',
      units: 'metric',
      baseLayer: 'norgeskart',
      threeD: false,
      layers: false,
      following: false,
      setTheme: (theme) => set({ theme }),
      setUnits: (units) => set({ units }),
      setBaseLayer: (baseLayer) => set({ baseLayer }),
      setThreeD: (threeD) => set({ threeD }),
      setLayers: (layers) => set({ layers }),
      setFollowing: (following) => set({ following }),
    }),
    { name: 'turbo-ui', partialize: (s) => ({ theme: s.theme, units: s.units, baseLayer: s.baseLayer }) },
  ),
);
