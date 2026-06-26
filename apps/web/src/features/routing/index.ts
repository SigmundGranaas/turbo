/** `routing` feature slice — the route-planning tool. Two parts: an
 *  always-mounted `RouteController` (SSE solver + map overlay, gated on the
 *  store so it survives while the planner is hidden) and the slotted
 *  `RoutePlannerPanel` (the `route` mutex slot). The tool's engaged/overlay/
 *  solve state lives in `useRouting`; the host owns visibility (openRouting/
 *  closeRouting) and the routing→tracks "save route as a track" step. */
export { RouteController } from './RouteController';
export { RoutePlannerPanel } from './RoutePlannerPanel';
export { useRouting } from './routingStore';
export { openRouting, closeRouting } from './actions';
export { ROUTE_PROFILES, ROUTE_PRESETS, type RoutePlan, type RouteProfile, type RoutePresetKey } from './api';
