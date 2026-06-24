import { create } from 'zustand';

/** Which side panel is open over the map. The single selection model the
 *  detail/editor panels render from (port doc 06; one-selection-one-panel). */
export type PanelMode = 'none' | 'detail' | 'new' | 'edit';

interface DraftPoint {
  lat: number;
  lng: number;
  name: string;
}

interface SelectionState {
  mode: PanelMode;
  selectedId?: string;
  draft?: DraftPoint;
  openDetail: (id: string) => void;
  openEdit: (id: string) => void;
  openNew: (lat: number, lng: number, name: string) => void;
  close: () => void;
}

export const useSelection = create<SelectionState>((set) => ({
  mode: 'none',
  openDetail: (selectedId) => set({ mode: 'detail', selectedId }),
  openEdit: (selectedId) => set({ mode: 'edit', selectedId }),
  openNew: (lat, lng, name) => set({ mode: 'new', draft: { lat, lng, name }, selectedId: undefined }),
  close: () => set({ mode: 'none', selectedId: undefined, draft: undefined }),
}));
