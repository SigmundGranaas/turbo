import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import type { BaseLayerId, CustomBaseLayer } from '../baseLayers';
import { setCustomBaseLayers } from '../map-engine/scene';

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
  /** 3D-terrain slider level `[0, MAX_3D_EXAGGERATION]`; 0 = flat 2D (tilt
   *  locked). Session-only (not persisted) — a reload restores 3D from the saved
   *  camera pitch instead. Drives the derived scene environment. */
  threeDLevel: number;
  layers: boolean;
  following: boolean;
  /** Far-distance atmospheric haze in 3D (persisted). Off by default. */
  distanceHaze: boolean;
  /** My-position dot colour (CSS hex, persisted); undefined = the default blue. */
  locationDotColor?: string;
  /** User-added XYZ basemaps (persisted). Registered with the scene resolver
   *  by MapScreen; selecting one sets [baseLayer] to its id. */
  customLayers: CustomBaseLayer[];
  /** Last camera pose (persisted) — restored on the next load. */
  camera?: SavedCamera;
  setTheme: (t: ThemeMode) => void;
  setUnits: (u: Units) => void;
  setBaseLayer: (b: BaseLayerId) => void;
  setThreeDLevel: (v: number) => void;
  setLayers: (v: boolean) => void;
  setFollowing: (v: boolean) => void;
  setDistanceHaze: (v: boolean) => void;
  setLocationDotColor: (c: string | undefined) => void;
  addCustomLayer: (l: CustomBaseLayer) => void;
  removeCustomLayer: (id: string) => void;
  setCamera: (c: SavedCamera) => void;
}

export const useUiStore = create<UiState>()(
  persist(
    (set) => ({
      theme: 'system',
      units: 'metric',
      baseLayer: 'norgeskart',
      threeDLevel: 0,
      layers: false,
      following: false,
      distanceHaze: false,
      customLayers: [],
      setTheme: (theme) => set({ theme }),
      setUnits: (units) => set({ units }),
      setBaseLayer: (baseLayer) => set({ baseLayer }),
      setThreeDLevel: (threeDLevel) => set({ threeDLevel }),
      setLayers: (layers) => set({ layers }),
      setFollowing: (following) => set({ following }),
      setDistanceHaze: (distanceHaze) => set({ distanceHaze }),
      setLocationDotColor: (locationDotColor) => set({ locationDotColor }),
      addCustomLayer: (l) =>
        set((s) => ({ customLayers: [...s.customLayers.filter((c) => c.id !== l.id), l], baseLayer: l.id })),
      removeCustomLayer: (id) =>
        set((s) => ({
          customLayers: s.customLayers.filter((c) => c.id !== id),
          // Deleting the active custom basemap falls back to the default topo.
          ...(s.baseLayer === id ? { baseLayer: 'norgeskart' } : {}),
        })),
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
        customLayers: s.customLayers,
        camera: s.camera,
      }),
    },
  ),
);

// Keep the scene resolver's custom-basemap registry in sync with the persisted
// list. Module-scope (not a React effect) so a reload with a custom base
// selected registers it BEFORE the first scene build — child effects would run
// after MapSurface already built the initial scene and it would fall back to
// the default topo. The store owns persistence; scene.ts stays store-agnostic.
const syncCustomLayers = (layers: CustomBaseLayer[]) =>
  setCustomBaseLayers(
    Object.fromEntries(
      layers.map((c) => [c.id, { label: c.label, icon: 'public', url: c.url, maxZoom: c.maxZoom, attribution: c.label }]),
    ),
  );
syncCustomLayers(useUiStore.getState().customLayers);
useUiStore.subscribe((s) => syncCustomLayers(s.customLayers));
