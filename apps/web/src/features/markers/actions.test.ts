import { describe, it, expect, beforeEach } from 'vitest';
import { usePanelHost } from '../../map-core';
import { openMarkerDetail, openMarkerEditor, openNewMarker, closeMarker } from './actions';
import { useSelection } from './selectionStore';

/** Behaviour a user sees when interacting with markers: tapping a pin opens its
 *  detail, "edit" opens the editor on the same marker, dropping a new marker
 *  opens the new-marker editor with a draft, and dismissing clears it all. */
describe('marker actions', () => {
  beforeEach(() => {
    useSelection.getState().clear();
    usePanelHost.getState().close();
  });

  it('opening a marker shows its detail panel and selects it', () => {
    openMarkerDetail('m1');
    expect(usePanelHost.getState().active).toBe('marker-detail');
    expect(useSelection.getState().selectedId).toBe('m1');
  });

  it('editing shows the editor on the same marker', () => {
    openMarkerEditor('m1');
    expect(usePanelHost.getState().active).toBe('marker-edit');
    expect(useSelection.getState().selectedId).toBe('m1');
  });

  it('a new marker opens the editor with a draft point and no prior selection', () => {
    useSelection.getState().setSelected('old');
    openNewMarker(60.39, 5.32, 'Fishing spot');
    expect(usePanelHost.getState().active).toBe('marker-new');
    expect(useSelection.getState().draft).toEqual({ lat: 60.39, lng: 5.32, name: 'Fishing spot' });
    expect(useSelection.getState().selectedId).toBeUndefined();
  });

  it('opening a marker hides whatever panel was open (mutex)', () => {
    usePanelHost.getState().open('saved');
    openMarkerDetail('m1');
    expect(usePanelHost.getState().active).toBe('marker-detail');
  });

  it('closing dismisses the panel and clears the selection + draft', () => {
    openNewMarker(1, 2, 'x');
    closeMarker();
    expect(usePanelHost.getState().active).toBeNull();
    expect(useSelection.getState().selectedId).toBeUndefined();
    expect(useSelection.getState().draft).toBeUndefined();
  });
});
