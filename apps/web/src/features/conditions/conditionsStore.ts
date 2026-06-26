import { create } from 'zustand';

/** The conditions panel target — a point + label to show weather/ocean for. */
interface ConditionsState {
  target?: { lat: number; lng: number; name: string };
  open: (lat: number, lng: number, name: string) => void;
  close: () => void;
}

export const useConditionsPanel = create<ConditionsState>((set) => ({
  open: (lat, lng, name) => set({ target: { lat, lng, name } }),
  close: () => set({ target: undefined }),
}));
