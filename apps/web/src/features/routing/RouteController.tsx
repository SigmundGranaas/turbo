import { useEffect } from 'react';
import { RouteOverlay } from '../../map-core';
import { useRouting } from './routingStore';
import { planStream } from './api';

/** Always-mounted routing controller: runs the SSE solver whenever the route
 *  inputs change and draws the planned/preview line over the map. Gated on the
 *  routing store (not the panel mutex) so the overlay + solve survive while the
 *  planner panel is hidden behind another panel. */
export function RouteController() {
  const active = useRouting((s) => s.active);
  const waypoints = useRouting((s) => s.waypoints);
  const preset = useRouting((s) => s.preset);
  const profile = useRouting((s) => s.profile);
  const preview = useRouting((s) => s.preview);
  const plan = useRouting((s) => s.plan);

  // Run the SSE solver whenever the route inputs change; cancel stale streams.
  useEffect(() => {
    if (!active || waypoints.length < 2) {
      useRouting.getState().setPreview(null);
      return;
    }
    const ac = new AbortController();
    useRouting.getState().setStatus('solving');
    planStream(
      waypoints,
      preset,
      profile,
      {
        onProgress: (c) => useRouting.getState().setPreview(c),
        onResult: (p) => useRouting.getState().setPlan(p),
        onError: (msg) => useRouting.getState().setStatus('error', msg),
      },
      ac.signal,
    ).catch((e: Error) => {
      if (e.name !== 'AbortError') useRouting.getState().setStatus('error', 'Routing failed');
    });
    return () => ac.abort();
  }, [active, waypoints, preset, profile]);

  const coords = plan?.coords ?? preview ?? [];
  if (coords.length === 0 && waypoints.length === 0) return null;
  return (
    <RouteOverlay
      coords={coords}
      waypoints={waypoints}
      dashed={!plan}
      onWaypointDrag={(i, p) => useRouting.getState().updateWaypoint(i, p)}
    />
  );
}
