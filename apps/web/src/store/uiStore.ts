import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import type { BaseLayerId } from '../baseLayers';

export type ThemeMode = 'system' | 'light' | 'dark';
export type Units = 'metric' | 'imperial';

/** Last camera pose, persisted so a reload restores the view. */
export interface SavedCamera {
  lat: number;
  lng: number;
  zoom: number;
  bearing: number;
  pitch: number;
}

/** Client/session UI state. Theme + units are persisted (localStorage); the rest
 *  is per-session. Mirrors the bits Android keeps in MapUiState + Settings. */
interface UiState {
  theme: ThemeMode;
  units: Units;
  baseLayer: BaseLayerId;
  threeD: boolean;
  layers: boolean;
  following: boolean;
  /** Far-distance atmospheric haze in 3D (persisted). Off by default. */
  distanceHaze: boolean;
  /** My-position dot colour (CSS hex, persisted); undefined = the default blue. */
  locationDotColor?: string;
  /** Last camera pose (persisted) — restored on the next load. */
  camera?: SavedCamera;
  setTheme: (t: ThemeMode) => void;
  setUnits: (u: Units) => void;
  setBaseLayer: (b: BaseLayerId) => void;
  setThreeD: (v: boolean) => void;
  setLayers: (v: boolean) => void;
  setFollowing: (v: boolean) => void;
  setDistanceHaze: (v: boolean) => void;
  setLocationDotColor: (c: string | undefined) => void;
  setCamera: (c: SavedCamera) => void;
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
      distanceHaze: false,
      setTheme: (theme) => set({ theme }),
      setUnits: (units) => set({ units }),
      setBaseLayer: (baseLayer) => set({ baseLayer }),
      setThreeD: (threeD) => set({ threeD }),
      setLayers: (layers) => set({ layers }),
      setFollowing: (following) => set({ following }),
      setDistanceHaze: (distanceHaze) => set({ distanceHaze }),
      setLocationDotColor: (locationDotColor) => set({ locationDotColor }),
      setCamera: (camera) => set({ camera }),
    }),
    {
      name: 'turbo-ui',
      partialize: (s) => ({
        theme: s.theme,
        units: s.units,
        baseLayer: s.baseLayer,
        distanceHaze: s.distanceHaze,
        locationDotColor: s.locationDotColor,
        camera: s.camera,
      }),
    },
  ),
);
