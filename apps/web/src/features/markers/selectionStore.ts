import { create } from 'zustand';

interface DraftPoint {
  lat: number;
  lng: number;
  name: string;
}

/** Markers selection PAYLOAD — which marker is selected (detail/edit) + the
 *  new-marker draft point. WHICH marker panel is visible is the host panel mutex
 *  (`marker-detail` / `marker-edit` / `marker-new`), not this store; the
 *  pin-highlight derives from the active panel. */
interface SelectionState {
  selectedId?: string;
  draft?: DraftPoint;
  setSelected: (id?: string) => void;
  setDraft: (d?: DraftPoint) => void;
  clear: () => void;
}

export const useSelection = create<SelectionState>((set) => ({
  setSelected: (selectedId) => set({ selectedId }),
  setDraft: (draft) => set({ draft }),
  clear: () => set({ selectedId: undefined, draft: undefined }),
}));
