import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import type { BaseLayerId } from '../map/scene';

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
  sun: boolean;
  following: boolean;
  accountOpen: boolean;
  setTheme: (t: ThemeMode) => void;
  setUnits: (u: Units) => void;
  setBaseLayer: (b: BaseLayerId) => void;
  setThreeD: (v: boolean) => void;
  setLayers: (v: boolean) => void;
  setSun: (v: boolean) => void;
  setFollowing: (v: boolean) => void;
  openAccount: () => void;
  closeAccount: () => void;
}

export const useUiStore = create<UiState>()(
  persist(
    (set) => ({
      theme: 'system',
      units: 'metric',
      baseLayer: 'norgeskart',
      threeD: false,
      layers: false,
      sun: false,
      following: false,
      accountOpen: false,
      setTheme: (theme) => set({ theme }),
      setUnits: (units) => set({ units }),
      setBaseLayer: (baseLayer) => set({ baseLayer }),
      setThreeD: (threeD) => set({ threeD }),
      setLayers: (layers) => set({ layers }),
      setSun: (sun) => set({ sun }),
      setFollowing: (following) => set({ following }),
      openAccount: () => set({ accountOpen: true }),
      closeAccount: () => set({ accountOpen: false }),
    }),
    { name: 'turbo-ui', partialize: (s) => ({ theme: s.theme, units: s.units, baseLayer: s.baseLayer }) },
  ),
);
