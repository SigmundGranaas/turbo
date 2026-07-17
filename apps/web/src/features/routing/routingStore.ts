import { create } from 'zustand';
import type { LatLng } from '../../geo';
import type { RoutePlan, RoutePresetKey, RouteProfile } from './api';

export type RouteStatus = 'idle' | 'solving' | 'done' | 'error';

/** The route-planning tool's state. Opening it is mutually exclusive with the
 *  marker detail/editor panel (the host closes the other selection). */
interface RoutingState {
  active: boolean;
  profile: RouteProfile;
  preset: RoutePresetKey;
  roundTrip: boolean;
  waypoints: LatLng[];
  preview: LatLng[] | null;
  plan: RoutePlan | null;
  status: RouteStatus;
  error?: string;
  open: (dest?: LatLng) => void;
  close: () => void;
  addWaypoint: (p: LatLng) => void;
  updateWaypoint: (i: number, p: LatLng) => void;
  removeWaypoint: (i: number) => void;
  moveWaypoint: (from: number, to: number) => void;
  clear: () => void;
  setProfile: (p: RouteProfile) => void;
  setPreset: (p: RoutePresetKey) => void;
  setRoundTrip: (on: boolean) => void;
  setPreview: (c: LatLng[] | null) => void;
  setPlan: (p: RoutePlan) => void;
  setStatus: (s: RouteStatus, error?: string) => void;
}

export const useRouting = create<RoutingState>((set) => ({
  active: false,
  profile: 'foot',
  preset: 'balanced',
  roundTrip: false,
  waypoints: [],
  preview: null,
  plan: null,
  status: 'idle',
  open: (dest) =>
    set({ active: true, waypoints: dest ? [dest] : [], roundTrip: false, preview: null, plan: null, status: 'idle', error: undefined }),
  close: () => set({ active: false, waypoints: [], roundTrip: false, preview: null, plan: null, status: 'idle', error: undefined }),
  addWaypoint: (p) => set((s) => ({ waypoints: [...s.waypoints, p], plan: null, preview: null })),
  updateWaypoint: (i, p) =>
    set((s) => ({ waypoints: s.waypoints.map((w, j) => (j === i ? p : w)), plan: null, preview: null })),
  removeWaypoint: (i) => set((s) => ({ waypoints: s.waypoints.filter((_, j) => j !== i), plan: null, preview: null })),
  moveWaypoint: (from, to) =>
    set((s) => {
      if (from === to || from < 0 || to < 0 || from >= s.waypoints.length || to >= s.waypoints.length) return {};
      const next = s.waypoints.slice();
      const [moved] = next.splice(from, 1);
      next.splice(to, 0, moved);
      return { waypoints: next, plan: null, preview: null };
    }),
  clear: () => set({ waypoints: [], preview: null, plan: null, status: 'idle', error: undefined }),
  setProfile: (profile) => set({ profile }),
  setPreset: (preset) => set({ preset }),
  setRoundTrip: (roundTrip) => set({ roundTrip, plan: null, preview: null }),
  setPreview: (preview) => set({ preview }),
  setPlan: (plan) => set({ plan, status: 'done' }),
  setStatus: (status, error) => set({ status, error }),
}));
