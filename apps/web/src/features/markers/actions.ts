import { usePanelHost } from '../../map-core';
import { useSelection } from './selectionStore';

/** Marker tool actions — set the selection payload AND make the right marker
 *  panel the active one (host mutex). The host calls these; cross-feature
 *  closing (routing/paths) stays in the host. */
export const openMarkerDetail = (id: string) => {
  useSelection.getState().setSelected(id);
  usePanelHost.getState().open('marker-detail');
};

export const openMarkerEditor = (id: string) => {
  useSelection.getState().setSelected(id);
  usePanelHost.getState().open('marker-edit');
};

export const openNewMarker = (lat: number, lng: number, name: string) => {
  useSelection.getState().setDraft({ lat, lng, name });
  useSelection.getState().setSelected(undefined);
  usePanelHost.getState().open('marker-new');
};

/** Close the marker panel (clears the slot + the selection payload). */
export const closeMarker = () => {
  useSelection.getState().clear();
  usePanelHost.getState().close();
};
