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
  selectedCollectionId?: string;
  pickerItem?: CollectionItem; // add-to-collection target (marker/path)
  openList: () => void;
  setTab: (t: SavedTab) => void;
  openDetail: (id: string) => void;
  openCollection: (id: string) => void;
  openPicker: (item: CollectionItem) => void;
  closePicker: () => void;
  close: () => void;
}

export const usePaths = create<PathsState>((set) => ({
  open: false,
  tab: 'paths',
  openList: () => set({ open: true, selectedId: undefined, selectedCollectionId: undefined }),
  setTab: (tab) => set({ tab, selectedId: undefined, selectedCollectionId: undefined }),
  openDetail: (selectedId) => set({ open: true, selectedId, selectedCollectionId: undefined }),
  openCollection: (selectedCollectionId) => set({ open: true, tab: 'collections', selectedCollectionId, selectedId: undefined }),
  openPicker: (pickerItem) => set({ pickerItem }),
  closePicker: () => set({ pickerItem: undefined }),
  close: () => set({ open: false, selectedId: undefined, selectedCollectionId: undefined }),
}));
