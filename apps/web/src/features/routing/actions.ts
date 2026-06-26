import type { LatLng } from '../../geo';
import { usePanelHost } from '../../map-core';
import { useRouting } from './routingStore';

/** Routing tool actions — engage the tool (overlay + solver via the store) AND
 *  make the planner the active mutex panel. Opening it auto-hides the other
 *  mutex panels; closing it ends the tool session (clears the overlay) and the
 *  slot. The route overlay/solver live in the store, so they survive while the
 *  planner is hidden behind another panel. */
export function openRouting(dest?: LatLng) {
  useRouting.getState().open(dest);
  usePanelHost.getState().open('route');
}

export function closeRouting() {
  useRouting.getState().close();
  usePanelHost.getState().close();
}
