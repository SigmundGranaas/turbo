import { create } from 'zustand';

/** The single side-panel slot — the host's visibility authority. Exactly one
 *  panel is shown, or none; opening one replaces (hides) the previous, so
 *  exclusivity is intrinsic (no precedence cascade, no "close the others").
 *  It owns ONLY which panel is visible — never any feature's content/state,
 *  which lives in each slice's own store and survives the panel being hidden.
 *
 *  `PanelId` is an open string (the host narrows it in its render switch); the
 *  kernel stays feature-agnostic. Features ask via `open(id)` / `close()`. */
export type PanelId = string;

interface PanelHost {
  active: PanelId | null;
  open: (id: PanelId) => void;
  close: () => void;
}

export const usePanelHost = create<PanelHost>((set) => ({
  active: null,
  open: (active) => set({ active }),
  close: () => set({ active: null }),
}));
