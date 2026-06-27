import { useCallback, useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import type { TurboMap } from 'turbomap-web';
import { useSession } from '../api/auth';
import { createShareLink, redeemLink, shareUrl } from '../api/sharing';
import { useUiStore } from '../store/uiStore';
import { usePaths } from '../store/pathsStore';
import { useToast } from '../store/toast';
import { searchPlaces, type PlaceHit } from '../api/places';
import { parseCoord } from '../geo';
import { MapSurface } from '../map-engine';
import { UserLocationLayer, RouteOverlay, MapPointMarkers, useMapPoints, usePanelHost } from '../map-core';
import { SunSlider, useSun } from '../features/sun';
import { LayerPicker } from './LayerPicker';
import { MapContextMenu, type ContextMenuTarget } from './MapContextMenu';
import { MarkerPins, MarkerDetailPanel, MarkerEditorPanel, useMarkers, useDeleteMarker, useSelection, openMarkerDetail, openMarkerEditor, openNewMarker, closeMarker, reverseGeocode } from '../features/markers';
import { useRouting, RouteController, RoutePlannerPanel, openRouting, closeRouting, ROUTE_PROFILES } from '../features/routing';
import { useTracks, useDeleteTrack, useCreateTrack, PathsListPanel, PathDetailPanel, TrackEditorPanel, type Track } from '../features/tracks';
import { useCollections, CollectionsListPanel, CollectionDetailPanel, CollectionPicker } from '../features/collections';
import type { CollectionItem } from '../api/collections';
import { AccountSettingsPanel } from '../features/account';
import { ConditionsPanel, useConditionsPanel } from '../features/conditions';
import { useResolvedDark } from '../theme/useTheme';
import { useIsMobile } from '../theme/useMedia';
import { NavRail } from '../ui/NavRail';
import { MobileNav } from '../ui/MobileNav';
import { SearchField } from '../ui/SearchField';
import { Glass, GlassIconBtn } from '../ui/Glass';
import { MapRail, MapReadout } from '../ui/MapControls';

const DPR = () => Math.min(window.devicePixelRatio || 1, 2);

interface Cam {
  lat: number;
  lng: number;
  zoom: number;
  pitch: number;
  bearing: number;
}

/** The map home — full-bleed map with the design's glass chrome, the marker
 *  overlay + side panels, and the routing tool. */
export function MapScreen() {
  const base = useUiStore((s) => s.baseLayer);
  const threeD = useUiStore((s) => s.threeD);
  const layers = useUiStore((s) => s.layers);
  const sun = useSun();
  const following = useUiStore((s) => s.following);
  const activePanel = usePanelHost((s) => s.active);

  const session = useSession();
  const markersQ = useMarkers();
  const del = useDeleteMarker();
  const sel = useSelection();
  const paths = usePaths();
  const tracksQ = useTracks();
  const delTrack = useDeleteTrack();
  const createTrack = useCreateTrack();
  const collectionsQ = useCollections();
  const conditions = useConditionsPanel();

  const dark = useResolvedDark();
  const isMobile = useIsMobile();
  const qc = useQueryClient();
  const [ready, setReady] = useState(false);
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<PlaceHit[]>([]);
  const toast = useToast((s) => s.message);
  const [ctxMenu, setCtxMenu] = useState<ContextMenuTarget | null>(null);
  const [shareToken] = useState(() => new URLSearchParams(window.location.search).get('share'));
  const mapRef = useRef<TurboMap | null>(null);
  // Initial camera: restore the last saved pose so a reload returns to the same
  // view (lat/lng/zoom seed `create`; pitch/bearing applied in onReady). Read
  // once at mount; `undefined` (first-ever load) → MapSurface's default, then
  // onReady flies to the user's location.
  const [initialCamera] = useState(() => {
    const c = useUiStore.getState().camera;
    return c ? { lat: c.lat, lng: c.lng, zoom: c.zoom } : undefined;
  });

  // Search: debounced place-name lookup (tileserver anchors) + "lat, lng"
  // coordinate parse. Results render in a dropdown under the search field;
  // selecting one eases the camera there.
  useEffect(() => {
    const coord = parseCoord(query);
    if (coord) {
      setResults([{ name: `${coord.lat.toFixed(4)}, ${coord.lng.toFixed(4)}`, kind: 'coordinate', ...coord }]);
      return;
    }
    if (query.trim().length < 2) {
      setResults([]);
      return;
    }
    const t = setTimeout(() => {
      void searchPlaces(query).then(setResults).catch(() => setResults([]));
    }, 250);
    return () => clearTimeout(t);
  }, [query]);

  const flyTo = (lat: number, lng: number) => {
    const m = mapRef.current;
    if (!m) return;
    m.ease_to(lat, lng, 14, 0, 800);
    setQuery('');
    setResults([]);
  };

  const showToast = (msg: string) => useToast.getState().show(msg);

  const shareResource = async (id: string) => {
    try {
      const token = await createShareLink(id);
      await navigator.clipboard.writeText(shareUrl(token));
      showToast('Share link copied to clipboard');
    } catch {
      showToast('Sign in to share');
    }
  };

  // Mobile bottom-sheet detents (peek / half / full), drag the grab handle.
  const DETENTS = ['40vh', '64vh', '88vh'];
  const [detent, setDetent] = useState(1);
  const [dragH, setDragH] = useState<number | null>(null);
  const sheetRef = useRef<HTMLDivElement>(null);
  const dragRef = useRef<{ startY: number; startH: number } | null>(null);
  const onHandleDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    const h = sheetRef.current?.getBoundingClientRect().height ?? 0;
    dragRef.current = { startY: e.clientY, startH: h };
    e.currentTarget.setPointerCapture(e.pointerId);
  };
  const onHandleMove = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (!dragRef.current) return;
    const dy = dragRef.current.startY - e.clientY;
    setDragH(Math.max(140, Math.min(window.innerHeight * 0.92, dragRef.current.startH + dy)));
  };
  const onHandleUp = () => {
    if (!dragRef.current) return;
    const h = dragH ?? dragRef.current.startH;
    const targets = [0.4, 0.64, 0.88].map((f) => f * window.innerHeight);
    let best = 0;
    let bd = Infinity;
    targets.forEach((t, i) => {
      const d = Math.abs(t - h);
      if (d < bd) {
        bd = d;
        best = i;
      }
    });
    setDetent(best);
    setDragH(null);
    dragRef.current = null;
  };

  const markers = markersQ.data ?? [];
  const tracks = tracksQ.data ?? [];
  const collections = collectionsQ.data ?? [];
  const selectedMarker = sel.selectedId ? markers.find((m) => m.id === sel.selectedId) : undefined;
  const selectedTrack = paths.selectedId ? tracks.find((t) => t.id === paths.selectedId) : undefined;
  const selectedCollection = paths.selectedCollectionId ? collections.find((c) => c.id === paths.selectedCollectionId) : undefined;
  const resolveItemName = (item: CollectionItem) =>
    item.type === 'marker'
      ? markers.find((m) => m.id === item.uuid)?.name || 'Marker'
      : tracks.find((t) => t.id === item.uuid)?.name || 'Path';
  // Every side panel is now one mutex slot (account/conditions/route/marker-*/
  // saved) — exactly one can be active, so visibility is a plain equality, no
  // precedence cascade. The routing OVERLAY/solve persists independently via the
  // store (RouteController), so it can outlive a hidden planner panel.
  const accountPanel = activePanel === 'account';
  const conditionsPanel = activePanel === 'conditions';
  const routePanel = activePanel === 'route';
  const markerPanel = activePanel === 'marker-detail' || activePanel === 'marker-edit' || activePanel === 'marker-new';
  const savedPanel = activePanel === 'saved';
  const panelShown = activePanel !== null;

  useEffect(() => {
    const root = document.documentElement;
    if (dark) root.setAttribute('data-theme', 'dark');
    else root.removeAttribute('data-theme');
  }, [dark]);

  // Show the click indicator (a ground ring) wherever the point menu is anchored,
  // and clear it when the menu closes — the visible "you clicked here" marker.
  useEffect(() => {
    useMapPoints.getState().setClick(ctxMenu ? { lat: ctxMenu.lat, lng: ctxMenu.lng } : null);
  }, [ctxMenu]);

  // Escape dismisses the topmost overlay: the point menu first, otherwise the
  // open side panel (+ its tool state). Standard overlay convention.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      if (ctxMenu) {
        setCtxMenu(null);
        return;
      }
      if (activePanel === null && !useRouting.getState().active) return;
      useRouting.getState().close();
      useConditionsPanel.getState().close();
      useSelection.getState().clear();
      usePaths.getState().reset();
      usePanelHost.getState().close();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [ctxMenu, activePanel]);

  // Desktop: reserve the side panel's width as a right viewport inset so the
  // map renders/centres in the visible band left of the panel (the focus point
  // never hides behind it). The engine shifts the projection — the camera isn't
  // moved — so markers/taps stay correct. Mobile sheets are handled separately.
  useEffect(() => {
    const m = mapRef.current;
    if (!m) return;
    // Panel is width 384 at right:84 → reserve 84+384 so the focus centres in
    // the visible band left of the panel.
    m.set_viewport_inset_right(!isMobile && panelShown ? 468 : 0);
  }, [panelShown, isMobile]);

  // Terrain sun-lighting (Lambertian shading + cast shadows + haze) belongs to
  // sun mode. Plain 3D draws the bare bright basemap over the relief — so a
  // 2D→3D switch doesn't darken the scene, and the heavy per-fragment shading
  // path is skipped (big perf win). No-op in 2D (no DEM). Re-applied when sun
  // toggles or the engine boots.
  useEffect(() => {
    mapRef.current?.set_terrain_lit(sun.on);
  }, [sun.on, ready]);

  // Redeem a ?share=<token> link on open. Requires sign-in (the grant is
  // materialised for the current user), so prompt the account panel if needed;
  // once signed in, redeem → the resource flows in via sync → open it.
  useEffect(() => {
    if (!shareToken) return;
    if (!session.data) {
      usePanelHost.getState().open('account');
      return;
    }
    redeemLink(shareToken)
      .then((res) => {
        qc.invalidateQueries({ queryKey: ['tracks'] });
        qc.invalidateQueries({ queryKey: ['markers'] });
        qc.invalidateQueries({ queryKey: ['collections'] });
        window.history.replaceState({}, '', window.location.pathname);
        showToast('Added to your library');
        const rt = (res.resourceType ?? '').toLowerCase();
        if (rt.includes('track') || rt.includes('path')) {
          usePaths.getState().openDetail(res.resourceId);
          usePanelHost.getState().open('saved');
        }
        else openMarkerDetail(res.resourceId);
      })
      .catch(() => showToast('That share link is invalid or expired'));
  }, [shareToken, session.data, qc]);

  const frameTrack = (t: Track) => {
    const m = mapRef.current;
    if (!m || t.points.length === 0) return;
    const lats = t.points.map((p) => p.lat);
    const lngs = t.points.map((p) => p.lng);
    const minLat = Math.min(...lats);
    const maxLat = Math.max(...lats);
    const minLng = Math.min(...lngs);
    const maxLng = Math.max(...lngs);
    const span = Math.max(maxLat - minLat, maxLng - minLng) || 0.01;
    const zoom = Math.max(8, Math.min(15, Math.log2(360 / span) - 1));
    m.ease_to((minLat + maxLat) / 2, (minLng + maxLng) / 2, zoom, 0, 700);
  };

  // Frame the camera to a track when it's selected from the list.
  useEffect(() => {
    if (selectedTrack) frameTrack(selectedTrack);
  }, [paths.selectedId, selectedTrack]);

  const onReady = useCallback((m: TurboMap) => {
    mapRef.current = m;
    setReady(true);
    const saved = useUiStore.getState().camera;
    if (saved) {
      // Restore the full pose — `create` only used lat/lng/zoom. If it was a 3D
      // view, flip threeD on so the terrain scene loads to match the pitch.
      m.set_camera(saved.lat, saved.lng, saved.zoom, saved.pitch, saved.bearing);
      if (saved.pitch > 0 && !useUiStore.getState().threeD) useUiStore.getState().setThreeD(true);
    } else if ('geolocation' in navigator) {
      // First-ever load: ease to the user's location (stays at the default if
      // permission is denied / unavailable).
      navigator.geolocation.getCurrentPosition(
        (pos) => {
          m.ease_to(pos.coords.latitude, pos.coords.longitude, 14, 0, 1000);
          useUiStore.getState().setFollowing(true);
        },
        () => {},
        { enableHighAccuracy: true, timeout: 8000 },
      );
    }
  }, []);

  // Persist the camera so a reload restores the view (throttled poll + on tab
  // hide). Reads the live engine pose; only writes when the map exists.
  useEffect(() => {
    const save = () => {
      const m = mapRef.current;
      if (!m) return;
      try {
        const c = JSON.parse(m.camera_json()) as Cam;
        useUiStore.getState().setCamera({ lat: c.lat, lng: c.lng, zoom: c.zoom, bearing: c.bearing, pitch: c.pitch });
      } catch {
        /* ignore */
      }
    };
    const id = setInterval(save, 1500);
    window.addEventListener('pagehide', save);
    return () => {
      clearInterval(id);
      window.removeEventListener('pagehide', save);
    };
  }, []);

  const cam = (): Cam | null => {
    const m = mapRef.current;
    if (!m) return null;
    try {
      return JSON.parse(m.camera_json()) as Cam;
    } catch {
      return null;
    }
  };

  const zoom = (factor: number) => {
    const m = mapRef.current;
    if (!m) return;
    const dpr = DPR();
    // Eased zoom about the viewport centre so the +/- buttons glide.
    m.zoom_around_animated(factor, (window.innerWidth * dpr) / 2, (window.innerHeight * dpr) / 2, 260);
  };
  const toggle3d = () => {
    const m = mapRef.current;
    const c = cam();
    if (!m || !c) return;
    const next = !threeD;
    m.set_camera(c.lat, c.lng, c.zoom, next ? 60 : 0, c.bearing);
    useUiStore.getState().setThreeD(next);
  };
  const recenter = () => {
    const m = mapRef.current;
    if (!m || !('geolocation' in navigator)) return;
    navigator.geolocation.getCurrentPosition(
      (pos) => {
        m.ease_to(pos.coords.latitude, pos.coords.longitude, 15, 0, 800);
        useUiStore.getState().setFollowing(true);
      },
      () => {},
      { enableHighAccuracy: true, timeout: 8000 },
    );
  };
  const resetNorth = () => {
    const m = mapRef.current;
    const c = cam();
    if (!m || !c) return;
    m.ease_to(c.lat, c.lng, c.zoom, 0, 500);
  };

  // Show the conditions panel for a point — sets its target (the panel payload)
  // and makes it the active panel via the host mutex.
  const showConditions = (lat: number, lng: number, name: string) => {
    useConditionsPanel.getState().open(lat, lng, name);
    usePanelHost.getState().open('conditions');
  };

  // The "Saved" panel is one mutex slot (`saved`); its tab/detail/edit
  // sub-navigation lives in `pathsStore` and survives while hidden. Opening it
  // via the mutex auto-hides the other mutex panels; closing resets the sub-nav.
  const openSaved = () => {
    usePaths.getState().openList();
    usePanelHost.getState().open('saved');
  };
  const closeSaved = () => {
    usePaths.getState().reset();
    usePanelHost.getState().close();
  };

  // Save the solved route as a track (a routing→tracks step the host owns), then
  // close the planner and open the new track's detail.
  const saveRouteAsTrack = () => {
    const r = useRouting.getState();
    if (!r.plan) return;
    const icon = ROUTE_PROFILES.find((p) => p.key === r.profile)?.icon;
    createTrack.mutate(
      {
        name: `Route · ${new Date().toLocaleDateString()}`,
        points: r.plan.coords,
        iconKey: icon,
        distanceM: r.plan.distanceM,
        ascentM: r.plan.ascentM,
        movingTimeS: Math.round(r.plan.durationS),
      },
      {
        onSuccess: (t) => {
          closeRouting();
          usePaths.getState().openDetail(t.id);
          usePanelHost.getState().open('saved');
        },
        onError: () => showToast('Couldn’t save the route. Sign in and try again.'),
      },
    );
  };

  const onNav = (id: string) => {
    useConditionsPanel.getState().close();
    useRouting.getState().close();
    useSelection.getState().clear();
    if (id === 'saved') {
      openSaved();
    } else if (id === 'conditions') {
      const c = cam();
      if (c) showConditions(c.lat, c.lng, 'Map centre');
    } else {
      usePanelHost.getState().close();
    }
  };

  // Open the new-marker editor at a geographic point (reverse-geocoded name).
  const createMarkerLatLng = async (lat: number, lng: number) => {
    useConditionsPanel.getState().close();
    openNewMarker(lat, lng, await reverseGeocode(lat, lng));
  };

  // Screen (CSS px) → geo, raycast onto the 3D relief so a click lands on the
  // mountainside the user actually sees in a tilted view — not the point
  // downhill where a flat-plane ray would reach sea level. Falls back to the
  // flat plane in 2D / before the DEM is resident.
  const groundLatLng = (x: number, y: number): { lat: number; lng: number } | null => {
    const m = mapRef.current;
    if (!m) return null;
    const g = m.unproject_ground(x * DPR(), y * DPR());
    if (!g || g.length < 2) return null;
    return { lat: g[0], lng: g[1] };
  };

  // Close whatever panel/selection is open. Called before a fresh map click so
  // clicking elsewhere always closes the previous panel first, then opens the
  // new one (never two open at once). Routing is left alone — its tool persists.
  const dismissPanels = () => {
    usePanelHost.getState().close();
    useConditionsPanel.getState().close();
    usePaths.getState().reset();
    useSelection.getState().clear();
  };

  // Open the point contextual menu (the Android long-press menu) at a screen
  // point: terrain-aware unproject to a geo point and anchor the menu there.
  const openContextMenu = (x: number, y: number) => {
    const g = groundLatLng(x, y);
    if (!g) return;
    setCtxMenu({ x, y, lat: g.lat, lng: g.lng });
  };

  // A tap/click on the map (the gesture controller already filtered out drags,
  // doubles, and long-presses). While routing, any tap adds a waypoint. Else a
  // mouse click closes any open panel then opens the point menu; a touch tap
  // dismisses an open menu/panel (touch opens the menu via long-press instead,
  // so it doesn't fight double-tap-zoom).
  const onMapTap = (x: number, y: number, pointerType: string) => {
    if (useRouting.getState().active) {
      const g = groundLatLng(x, y);
      if (g) useRouting.getState().addWaypoint(g);
      return;
    }
    if (pointerType === 'mouse') {
      // First click on the map closes whatever popup/panel is open — and opens
      // nothing. Only when nothing is open does a click open the point menu.
      if (ctxMenu || usePanelHost.getState().active) {
        setCtxMenu(null);
        dismissPanels();
        return;
      }
      openContextMenu(x, y);
      return;
    }
    // Touch tap on empty map → dismiss the menu / any open panel.
    setCtxMenu(null);
    dismissPanels();
  };

  // Long-press (touch) → the point menu, even while routing (tap still adds
  // waypoints). Mirrors the native long-press contextual menu.
  const onMapLongPress = (x: number, y: number) => {
    if (useRouting.getState().active) {
      const g = groundLatLng(x, y);
      if (g) useRouting.getState().addWaypoint(g);
      return;
    }
    dismissPanels();
    openContextMenu(x, y);
  };

  const onPinSelect = (id: string) => {
    if (useRouting.getState().active) {
      const mk = markers.find((x) => x.id === id);
      if (mk) useRouting.getState().addWaypoint({ lat: mk.lat, lng: mk.lng });
    } else {
      setCtxMenu(null);
      useConditionsPanel.getState().close();
      openMarkerDetail(id);
    }
  };

  const routeHere = (lat: number, lng: number) => {
    useConditionsPanel.getState().close();
    useSelection.getState().clear();
    openRouting({ lat, lng });
  };

  const onAccount = () => {
    useRouting.getState().close();
    useConditionsPanel.getState().close();
    useSelection.getState().clear();
    usePanelHost.getState().open('account');
  };
  const avatar = session.data ? (session.data.name ?? session.data.email ?? 'S').trim().charAt(0).toUpperCase() : undefined;

  return (
    <div style={{ position: 'fixed', inset: 0, overflow: 'hidden', background: 'var(--surface)' }}>
      <MapSurface
        base={base}
        threeD={threeD}
        camera={initialCamera}
        onReady={onReady}
        onEnter3d={() => useUiStore.getState().setThreeD(true)}
        onTap={onMapTap}
        onLongPress={onMapLongPress}
      />
      <RouteController />
      <MapPointMarkers />
      {savedPanel && selectedTrack && selectedTrack.points.length > 0 && (
        <RouteOverlay
          coords={selectedTrack.points}
          waypoints={[selectedTrack.points[0], selectedTrack.points[selectedTrack.points.length - 1]]}
          color={selectedTrack.colorHex || 'var(--primary)'}
        />
      )}
      <MarkerPins markers={markers} selectedId={markerPanel ? sel.selectedId : undefined} onSelect={onPinSelect} />
      <UserLocationLayer />

      {/* left: app-shell nav rail (desktop) */}
      {!isMobile && (
        <div style={{ position: 'absolute', top: 16, left: 16, bottom: 16, zIndex: 10 }}>
          <NavRail dark={dark} active={savedPanel ? 'saved' : 'explore'} signedIn={Boolean(session.data)} avatar={avatar ?? 'S'} onNav={onNav} onAccount={onAccount} />
        </div>
      )}

      {/* top: search pill + route-tool toggle (+ Saved on mobile, where there's no nav rail) */}
      <div
        style={
          isMobile
            ? { position: 'absolute', top: 8, left: 8, right: 8, zIndex: 10, display: 'flex', gap: 8 }
            : { position: 'absolute', top: 16, left: 96, zIndex: 10, display: 'flex', gap: 12 }
        }
      >
        <SearchField dark={dark} value={query} onChange={setQuery} width={isMobile ? '100%' : 420} avatar={avatar} onAvatar={onAccount} />
        {/* Routing is started from the map point menu (long-press / click),
            mirroring native — so there's no standalone route toggle here. */}
        {isMobile && (
          <Glass dark={dark} radius={9999} style={{ padding: 4, display: 'flex' }}>
            <GlassIconBtn icon="bookmark" active={savedPanel} title="Saved" onClick={() => onNav('saved')} />
          </Glass>
        )}
      </div>

      {/* search results dropdown */}
      {results.length > 0 && query.trim().length >= 2 && (
        <div
          className="tm-pop"
          style={{
            position: 'absolute',
            top: isMobile ? 58 : 62,
            left: isMobile ? 8 : 96,
            width: isMobile ? 'calc(100% - 16px)' : 420,
            zIndex: 12,
          }}
        >
          <Glass dark={dark} radius={16} style={{ padding: 6, display: 'flex', flexDirection: 'column', gap: 2, maxHeight: 360, overflowY: 'auto' }}>
            {results.map((r, i) => (
              <button
                key={`${r.name}-${i}`}
                onClick={() => flyTo(r.lat, r.lng)}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: 12,
                  width: '100%',
                  padding: '10px 12px',
                  borderRadius: 12,
                  border: 'none',
                  cursor: 'pointer',
                  textAlign: 'left',
                  background: 'transparent',
                  color: 'var(--on-surface)',
                }}
              >
                <span className="material-symbols-outlined" style={{ fontSize: 20, color: 'var(--primary)' }}>
                  {r.kind === 'coordinate' ? 'my_location' : 'place'}
                </span>
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ font: '500 15px/19px var(--font-sans)', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{r.name}</div>
                  <div style={{ font: '400 12px/15px var(--font-sans)', color: 'var(--on-surface-variant)', textTransform: 'capitalize' }}>
                    {r.kind.replace(/_/g, ' ')}
                  </div>
                </div>
              </button>
            ))}
          </Glass>
        </div>
      )}

      {/* bottom-right: map control cluster (hidden on mobile when a sheet is up) */}
      <div
        style={{
          position: 'absolute',
          // Action buttons stay pinned to the right edge; the side panel sits to
          // their LEFT (panel right:84), so they never overlap. Mobile hides the
          // cluster under a sheet instead.
          right: 16,
          // lift above the mobile bottom nav so the zoom buttons aren't covered
          bottom: isMobile ? 80 : 16,
          zIndex: 10,
          display: isMobile && panelShown ? 'none' : undefined,
        }}
      >
        {/* The picker pops to the LEFT of the rail, top-aligned with the
            layers button (the rail's top button) — absolute so it doesn't
            reflow/shove the rail. `right:64` = rail width (~52) + gap. */}
        {layers && (
          <div style={{ position: 'absolute', right: 64, top: 0 }}>
            <LayerPicker
              dark={dark}
              active={base}
              onSelect={(id) => {
                useUiStore.getState().setBaseLayer(id);
                useUiStore.getState().setLayers(false);
              }}
            />
          </div>
        )}
        <MapRail
          dark={dark}
          getBearing={() => cam()?.bearing ?? 0}
          state={{ layers, is3d: threeD, sun: sun.on, following }}
          on={{
            onLayers: () => useUiStore.getState().setLayers(!layers),
            onToggle3d: toggle3d,
            onSun: sun.toggle,
            onRecenter: recenter,
            onCompass: resetNorth,
            onZoomIn: () => zoom(1.4),
            onZoomOut: () => zoom(1 / 1.4),
          }}
        />
      </div>

      {/* bottom-left: coordinate / scale readout (desktop only) */}
      {!isMobile && (
        <div style={{ position: 'absolute', left: 96, bottom: 22, zIndex: 10 }}>
          <MapReadout />
        </div>
      )}

      {/* mobile bottom nav — the destinations the desktop NavRail holds
          (Conditions/Activities/Account have no other entry point on a phone).
          Hidden while a sheet is up, like the control cluster. */}
      {isMobile && !panelShown && (
        <div style={{ position: 'absolute', left: 8, right: 8, bottom: 8, zIndex: 10 }}>
          <MobileNav
            dark={dark}
            active="explore"
            signedIn={Boolean(session.data)}
            avatar={avatar ?? 'S'}
            onNav={onNav}
            onAccount={onAccount}
          />
        </div>
      )}

      {/* sun time-of-day slider — only while Sun mode is on; bottom-centred,
          lifted clear of the mobile nav. Hidden under a mobile sheet. */}
      {sun.on && !(isMobile && panelShown) && (
        <div
          style={{
            position: 'absolute',
            left: 0,
            right: 0,
            bottom: isMobile ? 150 : 24,
            display: 'flex',
            justifyContent: 'center',
            zIndex: 10,
            pointerEvents: 'none',
          }}
        >
          <div style={{ pointerEvents: 'auto' }}>
            <SunSlider dark={dark} hour={sun.hour} onChange={sun.setHour} />
          </div>
        </div>
      )}

      {/* right column (desktop) / bottom sheet (mobile): routing / detail / editor */}
      {panelShown && (
        <div
          ref={sheetRef}
          key={activePanel ?? 'panel'}
          className={isMobile ? 'tm-panel-mobile' : 'tm-panel-desktop'}
          style={
            isMobile
              ? { position: 'absolute', left: 8, right: 8, bottom: 'calc(8px + env(safe-area-inset-bottom))', height: dragH != null ? `${dragH}px` : DETENTS[detent], zIndex: 11 }
              : { position: 'absolute', top: 16, right: 84, bottom: 16, width: 384, zIndex: 11 }
          }
        >
          {isMobile && (
            <div
              onPointerDown={onHandleDown}
              onPointerMove={onHandleMove}
              onPointerUp={onHandleUp}
              title="Drag to resize"
              style={{ position: 'absolute', top: 0, left: 0, right: 0, height: 22, display: 'flex', justifyContent: 'center', alignItems: 'center', zIndex: 2, cursor: 'ns-resize', touchAction: 'none' }}
            >
              <div style={{ width: 36, height: 5, borderRadius: 3, background: 'var(--outline-variant)' }} />
            </div>
          )}
          {accountPanel && <AccountSettingsPanel dark={dark} onClose={() => usePanelHost.getState().close()} />}
          {conditionsPanel && conditions.target && (
            <ConditionsPanel
              dark={dark}
              lat={conditions.target.lat}
              lng={conditions.target.lng}
              name={conditions.target.name}
              onClose={() => usePanelHost.getState().close()}
            />
          )}
          {routePanel && (
            <RoutePlannerPanel
              dark={dark}
              onClose={closeRouting}
              onSaveAsTrack={saveRouteAsTrack}
              saving={createTrack.isPending}
            />
          )}
          {markerPanel && activePanel === 'marker-detail' && selectedMarker && (
            <MarkerDetailPanel
              dark={dark}
              marker={selectedMarker}
              onEdit={() => openMarkerEditor(selectedMarker.id)}
              onRoute={() => routeHere(selectedMarker.lat, selectedMarker.lng)}
              onSave={() => usePaths.getState().openPicker({ type: 'marker', uuid: selectedMarker.id })}
              onShare={() => void shareResource(selectedMarker.id)}
              onConditions={() => showConditions(selectedMarker.lat, selectedMarker.lng, selectedMarker.name)}
              onDelete={() => del.mutate(selectedMarker, { onSuccess: () => closeMarker() })}
              onClose={() => closeMarker()}
            />
          )}
          {markerPanel && activePanel === 'marker-edit' && selectedMarker && (
            <MarkerEditorPanel dark={dark} marker={selectedMarker} onClose={() => closeMarker()} onSaved={(m) => openMarkerDetail(m.id)} />
          )}
          {markerPanel && activePanel === 'marker-new' && sel.draft && (
            <MarkerEditorPanel
              key={`${sel.draft.lat},${sel.draft.lng}`}
              dark={dark}
              point={sel.draft}
              onClose={() => closeMarker()}
              onSaved={(m) => openMarkerDetail(m.id)}
            />
          )}
          {savedPanel &&
            (selectedTrack && paths.editingId === selectedTrack.id ? (
              <TrackEditorPanel
                dark={dark}
                track={selectedTrack}
                onClose={() => usePaths.getState().closeEdit()}
                onSaved={(t) => usePaths.getState().openDetail(t.id)}
              />
            ) : selectedTrack ? (
              <PathDetailPanel
                dark={dark}
                track={selectedTrack}
                onShow={() => frameTrack(selectedTrack)}
                onEdit={() => usePaths.getState().openEdit(selectedTrack.id)}
                onSave={() => usePaths.getState().openPicker({ type: 'path', uuid: selectedTrack.id })}
                onShare={() => void shareResource(selectedTrack.id)}
                onDelete={() => delTrack.mutate(selectedTrack, { onSuccess: () => usePaths.getState().openList() })}
                onBack={() => usePaths.getState().openList()}
                onClose={() => closeSaved()}
              />
            ) : selectedCollection ? (
              <CollectionDetailPanel
                dark={dark}
                collection={selectedCollection}
                resolveName={resolveItemName}
                onBack={() => usePaths.getState().openList()}
                onClose={() => closeSaved()}
              />
            ) : paths.tab === 'collections' ? (
              <CollectionsListPanel
                dark={dark}
                collections={collections}
                loading={collectionsQ.isLoading}
                onOpen={(id) => usePaths.getState().openCollection(id)}
                onClose={() => closeSaved()}
              />
            ) : (
              <PathsListPanel
                dark={dark}
                tracks={tracks}
                loading={tracksQ.isLoading}
                onSelect={(id) => usePaths.getState().openDetail(id)}
                onClose={() => closeSaved()}
              />
            ))}
        </div>
      )}

      {/* add-to-collection picker (modal over everything) */}
      {paths.pickerItem && (
        <CollectionPicker dark={dark} item={paths.pickerItem} onClose={() => usePaths.getState().closePicker()} />
      )}

      {ctxMenu && (
        <MapContextMenu
          dark={dark}
          target={ctxMenu}
          onNewMarker={() => void createMarkerLatLng(ctxMenu.lat, ctxMenu.lng)}
          onRouteHere={() =>
            useRouting.getState().active
              ? useRouting.getState().addWaypoint({ lat: ctxMenu.lat, lng: ctxMenu.lng })
              : routeHere(ctxMenu.lat, ctxMenu.lng)
          }
          onStartRoute={() => {
            useConditionsPanel.getState().close();
            useSelection.getState().clear();
            openRouting({ lat: ctxMenu.lat, lng: ctxMenu.lng });
          }}
          onForecast={(name) => showConditions(ctxMenu.lat, ctxMenu.lng, name)}
          onClose={() => setCtxMenu(null)}
        />
      )}

      {toast && (
        <div style={{ position: 'absolute', left: 0, right: 0, bottom: 24, zIndex: 40, display: 'flex', justifyContent: 'center', pointerEvents: 'none' }}>
          <div
            className="tm-toast"
            style={{
              background: 'var(--inverse-surface)',
              color: 'var(--surface)',
              padding: '12px 20px',
              borderRadius: 9999,
              font: '500 14px/1 var(--font-sans)',
              boxShadow: 'var(--elevation-3)',
            }}
          >
            {toast}
          </div>
        </div>
      )}

      {!ready && <div className="booting">Starting the map…</div>}
    </div>
  );
}
