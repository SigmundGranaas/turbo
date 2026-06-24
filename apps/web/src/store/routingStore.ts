import { create } from 'zustand';
import type { LatLng } from '../geo';
import type { RoutePlan, RoutePresetKey, RouteProfile } from '../api/routing';

export type RouteStatus = 'idle' | 'solving' | 'done' | 'error';

/** The route-planning tool's state. Opening it is mutually exclusive with the
 *  marker detail/editor panel (the host closes the other selection). */
interface RoutingState {
  active: boolean;
  profile: RouteProfile;
  preset: RoutePresetKey;
  waypoints: LatLng[];
  preview: LatLng[] | null;
  plan: RoutePlan | null;
  status: RouteStatus;
  error?: string;
  open: (dest?: LatLng) => void;
  close: () => void;
  addWaypoint: (p: LatLng) => void;
  removeWaypoint: (i: number) => void;
  clear: () => void;
  setProfile: (p: RouteProfile) => void;
  setPreset: (p: RoutePresetKey) => void;
  setPreview: (c: LatLng[] | null) => void;
  setPlan: (p: RoutePlan) => void;
  setStatus: (s: RouteStatus, error?: string) => void;
}

export const useRouting = create<RoutingState>((set) => ({
  active: false,
  profile: 'foot',
  preset: 'balanced',
  waypoints: [],
  preview: null,
  plan: null,
  status: 'idle',
  open: (dest) =>
    set({ active: true, waypoints: dest ? [dest] : [], preview: null, plan: null, status: 'idle', error: undefined }),
  close: () => set({ active: false, waypoints: [], preview: null, plan: null, status: 'idle', error: undefined }),
  addWaypoint: (p) => set((s) => ({ waypoints: [...s.waypoints, p], plan: null, preview: null })),
  removeWaypoint: (i) => set((s) => ({ waypoints: s.waypoints.filter((_, j) => j !== i), plan: null, preview: null })),
  clear: () => set({ waypoints: [], preview: null, plan: null, status: 'idle', error: undefined }),
  setProfile: (profile) => set({ profile }),
  setPreset: (preset) => set({ preset }),
  setPreview: (preview) => set({ preview }),
  setPlan: (plan) => set({ plan, status: 'done' }),
  setStatus: (status, error) => set({ status, error }),
}));
