import { create } from 'zustand';
import type { CollectionItem } from '../api/collections';

export type SavedTab = 'paths' | 'collections';

/** The "Saved" panel: a tabbed list of paths or collections, a selected path's
 *  detail, or a selected collection's detail. Also owns the cross-cutting
 *  add-to-collection picker target. */
interface PathsState {
  open: boolean;
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
  close: () => void;
}

export const usePaths = create<PathsState>((set) => ({
  open: false,
  tab: 'paths',
  openList: () => set({ open: true, selectedId: undefined, editingId: undefined, selectedCollectionId: undefined }),
  setTab: (tab) => set({ tab, selectedId: undefined, editingId: undefined, selectedCollectionId: undefined }),
  openDetail: (selectedId) => set({ open: true, selectedId, editingId: undefined, selectedCollectionId: undefined }),
  openEdit: (editingId) => set({ open: true, editingId, selectedId: editingId, selectedCollectionId: undefined }),
  closeEdit: () => set({ editingId: undefined }),
  openCollection: (selectedCollectionId) => set({ open: true, tab: 'collections', selectedCollectionId, selectedId: undefined, editingId: undefined }),
  openPicker: (pickerItem) => set({ pickerItem }),
  closePicker: () => set({ pickerItem: undefined }),
  close: () => set({ open: false, selectedId: undefined, editingId: undefined, selectedCollectionId: undefined }),
}));
