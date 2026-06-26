import { create } from 'zustand';
import type { CollectionItem } from '../api/collections';

export type SavedTab = 'paths' | 'collections';

/** Sub-navigation state for the host's "Saved" panel (the single `saved` mutex
 *  slot, shared by the `tracks` + `collections` slices): which tab, the selected
 *  path / collection, the path being edited, and the cross-cutting
 *  add-to-collection picker target. WHETHER the Saved panel is visible is the
 *  host panel mutex (`usePanelHost`), not this store — so the sub-nav survives
 *  while the panel is hidden (reopens to the same view). */
interface PathsState {
  tab: SavedTab;
  selectedId?: string; // path
  editingId?: string; // path being edited (overrides detail view)
  selectedCollectionId?: string;
  pickerItem?: CollectionItem; // add-to-collection target (marker/path)
  openList: () => void;
  setTab: (t: SavedTab) => void;
  openDetail: (id: string) => void;
  openEdit: (id: string) => void;
  closeEdit: () => void;
  openCollection: (id: string) => void;
  openPicker: (item: CollectionItem) => void;
  closePicker: () => void;
  reset: () => void;
}

export const usePaths = create<PathsState>((set) => ({
  tab: 'paths',
  openList: () => set({ selectedId: undefined, editingId: undefined, selectedCollectionId: undefined }),
  setTab: (tab) => set({ tab, selectedId: undefined, editingId: undefined, selectedCollectionId: undefined }),
  openDetail: (selectedId) => set({ selectedId, editingId: undefined, selectedCollectionId: undefined }),
  openEdit: (editingId) => set({ editingId, selectedId: editingId, selectedCollectionId: undefined }),
  closeEdit: () => set({ editingId: undefined }),
  openCollection: (selectedCollectionId) => set({ tab: 'collections', selectedCollectionId, selectedId: undefined, editingId: undefined }),
  openPicker: (pickerItem) => set({ pickerItem }),
  closePicker: () => set({ pickerItem: undefined }),
  reset: () => set({ tab: 'paths', selectedId: undefined, editingId: undefined, selectedCollectionId: undefined }),
}));
