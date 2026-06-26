import { useCallback, useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import type { TurboMap } from 'turbomap-web';
import { useSession } from '../api/auth';
import { createShareLink, redeemLink, shareUrl } from '../api/sharing';
import { useUiStore } from '../store/uiStore';
import { useSelection } from '../store/selectionStore';
import { useRouting } from '../store/routingStore';
import { usePaths } from '../store/pathsStore';
import { reverseGeocode } from '../api/markers';
import type { Track } from '../api/tracks';
import { planStream } from '../api/routing';
import { searchPlaces, type PlaceHit } from '../api/places';
import { parseCoord } from '../geo';
import { MapSurface } from '../map-engine';
import { MapEngineProvider, UserLocationLayer } from '../map-core';
import { LayerPicker } from './LayerPicker';
import { MapContextMenu, type ContextMenuTarget } from './MapContextMenu';
import { useMarkers, useDeleteMarker } from './markers/useMarkers';
import { useTracks, useDeleteTrack } from './paths/useTracks';
import { MarkerPins } from './markers/MarkerPins';
import { MarkerDetailPanel } from './markers/MarkerDetailPanel';
import { MarkerEditorPanel } from './markers/MarkerEditorPanel';
import { SunSlider } from './SunSlider';
import { RouteOverlay } from './routing/RouteOverlay';
import { RoutePlannerPanel } from './routing/RoutePlannerPanel';
import { PathsListPanel } from './paths/PathsListPanel';
import { PathDetailPanel } from './paths/PathDetailPanel';
import { TrackEditorPanel } from './paths/TrackEditorPanel';
import { useCollections } from './collections/useCollections';
import { CollectionsListPanel } from './collections/CollectionsListPanel';
import { CollectionDetailPanel } from './collections/CollectionDetailPanel';
import { CollectionPicker } from './collections/CollectionPicker';
import type { CollectionItem } from '../api/collections';
import { AccountSettingsPanel } from '../account/AccountSettingsPanel';
import { ConditionsPanel } from './conditions/ConditionsPanel';
import { useConditionsPanel } from '../store/conditionsStore';
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
  const sun = useUiStore((s) => s.sun);
  const following = useUiStore((s) => s.following);
  const accountOpen = useUiStore((s) => s.accountOpen);

  const session = useSession();
  const markersQ = useMarkers();
  const del = useDeleteMarker();
  const sel = useSelection();
  const routing = useRouting();
  const paths = usePaths();
  const tracksQ = useTracks();
  const delTrack = useDeleteTrack();
  const collectionsQ = useCollections();
  const conditions = useConditionsPanel();

  const dark = useResolvedDark();
  const isMobile = useIsMobile();
  const qc = useQueryClient();
  const [ready, setReady] = useState(false);
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<PlaceHit[]>([]);
  const [toast, setToast] = useState<string | null>(null);
  const [ctxMenu, setCtxMenu] = useState<ContextMenuTarget | null>(null);
  const [shareToken] = useState(() => new URLSearchParams(window.location.search).get('share'));
  const mapRef = useRef<TurboMap | null>(null);
  // Time-of-day (hours past local midnight) for the sun slider; seeded to now.
  const [sunHour, setSunHour] = useState(() => {
    const d = new Date();
    return d.getHours() + d.getMinutes() / 60;
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

  const showToast = (msg: string) => {
    setToast(msg);
    window.setTimeout(() => setToast((t) => (t === msg ? null : t)), 2600);
  };

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
  const accountPanel = accountOpen;
  const conditionsPanel = !accountPanel && Boolean(conditions.target);
  const routePanel = !accountPanel && !conditionsPanel && routing.active;
  const markerPanel = !accountPanel && !conditionsPanel && !routePanel && sel.mode !== 'none';
  const pathsPanel = !accountPanel && !conditionsPanel && !routePanel && !markerPanel && paths.open;
  const panelShown = accountPanel || conditionsPanel || routePanel || markerPanel || pathsPanel;

  useEffect(() => {
    const root = document.documentElement;
    if (dark) root.setAttribute('data-theme', 'dark');
    else root.removeAttribute('data-theme');
  }, [dark]);

  // Desktop: reserve the side panel's width as a right viewport inset so the
  // map renders/centres in the visible band left of the panel (the focus point
  // never hides behind it). The engine shifts the projection — the camera isn't
  // moved — so markers/taps stay correct. Mobile sheets are handled separately.
  useEffect(() => {
    const m = mapRef.current;
    if (!m) return;
    m.set_viewport_inset_right(!isMobile && panelShown ? 400 : 0);
  }, [panelShown, isMobile]);

  // Redeem a ?share=<token> link on open. Requires sign-in (the grant is
  // materialised for the current user), so prompt the account panel if needed;
  // once signed in, redeem → the resource flows in via sync → open it.
  useEffect(() => {
    if (!shareToken) return;
    if (!session.data) {
      useUiStore.getState().openAccount();
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
        if (rt.includes('track') || rt.includes('path')) usePaths.getState().openDetail(res.resourceId);
        else useSelection.getState().openDetail(res.resourceId);
      })
      .catch(() => showToast('That share link is invalid or expired'));
  }, [shareToken, session.data, qc]);

  // Run the SSE solver whenever the route inputs change; cancel stale streams.
  useEffect(() => {
    if (!routing.active || routing.waypoints.length < 2) {
      useRouting.getState().setPreview(null);
      return;
    }
    const ac = new AbortController();
    useRouting.getState().setStatus('solving');
    planStream(
      routing.waypoints,
      routing.preset,
      routing.profile,
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
  }, [routing.active, routing.waypoints, routing.preset, routing.profile]);

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

  const ensure3d = (m: TurboMap, c: Cam) => {
    if (!threeD) {
      m.set_camera(c.lat, c.lng, c.zoom, 60, c.bearing);
      useUiStore.getState().setThreeD(true);
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
  // Apply a time-of-day (hours past local midnight) as the engine sun time.
  const applySunHour = (h: number) => {
    const m = mapRef.current;
    if (!m) return;
    const d = new Date();
    d.setHours(Math.floor(h), Math.round((h - Math.floor(h)) * 60), 0, 0);
    m.set_sun_time(d.getTime() / 1000);
  };
  const toggleSun = () => {
    const m = mapRef.current;
    const c = cam();
    if (!m || !c) return;
    const next = !sun;
    if (next) {
      ensure3d(m, c);
      const d = new Date();
      const h = d.getHours() + d.getMinutes() / 60;
      setSunHour(h); // start at the real clock, then the slider sweeps it
      applySunHour(h);
      // Cast shadows (peaks shadow the valleys) — what makes the relief read as
      // distinct, like the native app. Off by default; only on in sun mode.
      m.set_terrain_shadows(0.7);
    } else {
      m.set_sun_time(undefined);
      m.set_terrain_shadows(0);
    }
    useUiStore.getState().setSun(next);
  };
  const onSunHour = (h: number) => {
    setSunHour(h);
    applySunHour(h);
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

  const onNav = (id: string) => {
    useUiStore.getState().closeAccount();
    useConditionsPanel.getState().close();
    useRouting.getState().close();
    useSelection.getState().close();
    if (id === 'saved') {
      usePaths.getState().openList();
    } else if (id === 'conditions') {
      usePaths.getState().close();
      const c = cam();
      if (c) useConditionsPanel.getState().open(c.lat, c.lng, 'Map centre');
    } else {
      usePaths.getState().close();
    }
  };

  // Open the new-marker editor at a geographic point (reverse-geocoded name).
  const createMarkerLatLng = async (lat: number, lng: number) => {
    useUiStore.getState().closeAccount();
    useConditionsPanel.getState().close();
    usePaths.getState().close();
    useSelection.getState().openNew(lat, lng, await reverseGeocode(lat, lng));
  };

  // Open the point contextual menu (the Android long-press menu) at a screen
  // point: unproject to a geo point and anchor the menu there.
  const openContextMenu = (x: number, y: number) => {
    const m = mapRef.current;
    if (!m) return;
    const g = m.unproject(x * DPR(), y * DPR());
    if (!g) return;
    setCtxMenu({ x, y, lat: g[0], lng: g[1] });
  };

  // A tap/click on the map (the gesture controller already filtered out drags,
  // doubles, and long-presses). While routing, any tap adds a waypoint. Else a
  // mouse click opens the point menu; a touch tap dismisses an open menu/panel
  // (touch opens the menu via long-press instead, so it doesn't fight
  // double-tap-zoom).
  const onMapTap = (x: number, y: number, pointerType: string) => {
    const m = mapRef.current;
    if (!m) return;
    if (useRouting.getState().active) {
      const g = m.unproject(x * DPR(), y * DPR());
      if (g) useRouting.getState().addWaypoint({ lat: g[0], lng: g[1] });
      return;
    }
    if (pointerType === 'mouse') {
      openContextMenu(x, y);
      return;
    }
    // Touch tap on empty map → dismiss the menu / any open panel.
    setCtxMenu(null);
    useUiStore.getState().closeAccount();
    useConditionsPanel.getState().close();
    usePaths.getState().close();
    useSelection.getState().close();
  };

  // Long-press (touch) → the point menu, even while routing (tap still adds
  // waypoints). Mirrors the native long-press contextual menu.
  const onMapLongPress = (x: number, y: number) => {
    if (useRouting.getState().active) {
      const m = mapRef.current;
      const g = m?.unproject(x * DPR(), y * DPR());
      if (g) useRouting.getState().addWaypoint({ lat: g[0], lng: g[1] });
      return;
    }
    openContextMenu(x, y);
  };

  const onPinSelect = (id: string) => {
    if (useRouting.getState().active) {
      const mk = markers.find((x) => x.id === id);
      if (mk) useRouting.getState().addWaypoint({ lat: mk.lat, lng: mk.lng });
    } else {
      useUiStore.getState().closeAccount();
      useConditionsPanel.getState().close();
      usePaths.getState().close();
      useSelection.getState().openDetail(id);
    }
  };

  const routeHere = (lat: number, lng: number) => {
    useUiStore.getState().closeAccount();
    useConditionsPanel.getState().close();
    useSelection.getState().close();
    usePaths.getState().close();
    useRouting.getState().open({ lat, lng });
  };

  const onAccount = () => {
    useRouting.getState().close();
    useConditionsPanel.getState().close();
    useSelection.getState().close();
    usePaths.getState().close();
    useUiStore.getState().openAccount();
  };
  const avatar = session.data ? (session.data.name ?? session.data.email ?? 'S').trim().charAt(0).toUpperCase() : undefined;

  const routeCoords = routing.plan?.coords ?? routing.preview ?? [];

  return (
    <MapEngineProvider>
    <div style={{ position: 'fixed', inset: 0, overflow: 'hidden', background: 'var(--surface)' }}>
      <MapSurface
        base={base}
        threeD={threeD}
        onReady={onReady}
        onEnter3d={() => useUiStore.getState().setThreeD(true)}
        onTap={onMapTap}
        onLongPress={onMapLongPress}
      />
      {(routeCoords.length > 0 || routing.waypoints.length > 0) && (
        <RouteOverlay coords={routeCoords} waypoints={routing.waypoints} dashed={!routing.plan} />
      )}
      {pathsPanel && selectedTrack && selectedTrack.points.length > 0 && (
        <RouteOverlay
          coords={selectedTrack.points}
          waypoints={[selectedTrack.points[0], selectedTrack.points[selectedTrack.points.length - 1]]}
          color={selectedTrack.colorHex || 'var(--primary)'}
        />
      )}
      <MarkerPins markers={markers} selectedId={sel.selectedId} onSelect={onPinSelect} />
      <UserLocationLayer />

      {/* left: app-shell nav rail (desktop) */}
      {!isMobile && (
        <div style={{ position: 'absolute', top: 16, left: 16, bottom: 16, zIndex: 10 }}>
          <NavRail dark={dark} active={paths.open ? 'saved' : 'explore'} signedIn={Boolean(session.data)} avatar={avatar ?? 'S'} onNav={onNav} onAccount={onAccount} />
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
            <GlassIconBtn icon="bookmark" active={paths.open} title="Saved" onClick={() => onNav('saved')} />
          </Glass>
        )}
      </div>

      {/* search results dropdown */}
      {results.length > 0 && query.trim().length >= 2 && (
        <div
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
          right: !isMobile && panelShown ? 416 : 16,
          // lift above the mobile bottom nav so the zoom buttons aren't covered
          bottom: isMobile ? 80 : 16,
          zIndex: 10,
          transition: 'right .25s var(--ease-out)',
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
          state={{ layers, is3d: threeD, sun, following }}
          on={{
            onLayers: () => useUiStore.getState().setLayers(!layers),
            onToggle3d: toggle3d,
            onSun: toggleSun,
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
      {sun && !(isMobile && panelShown) && (
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
            <SunSlider dark={dark} hour={sunHour} onChange={onSunHour} />
          </div>
        </div>
      )}

      {/* right column (desktop) / bottom sheet (mobile): routing / detail / editor */}
      {panelShown && (
        <div
          ref={sheetRef}
          style={
            isMobile
              ? { position: 'absolute', left: 8, right: 8, bottom: 8, height: dragH != null ? `${dragH}px` : DETENTS[detent], zIndex: 11 }
              : { position: 'absolute', top: 16, right: 16, bottom: 16, width: 384, zIndex: 11 }
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
          {accountPanel && <AccountSettingsPanel dark={dark} onClose={() => useUiStore.getState().closeAccount()} />}
          {conditionsPanel && conditions.target && (
            <ConditionsPanel
              dark={dark}
              lat={conditions.target.lat}
              lng={conditions.target.lng}
              name={conditions.target.name}
              onClose={() => useConditionsPanel.getState().close()}
            />
          )}
          {routePanel && <RoutePlannerPanel dark={dark} />}
          {markerPanel && sel.mode === 'detail' && selectedMarker && (
            <MarkerDetailPanel
              dark={dark}
              marker={selectedMarker}
              onEdit={() => useSelection.getState().openEdit(selectedMarker.id)}
              onRoute={() => routeHere(selectedMarker.lat, selectedMarker.lng)}
              onSave={() => usePaths.getState().openPicker({ type: 'marker', uuid: selectedMarker.id })}
              onShare={() => void shareResource(selectedMarker.id)}
              onConditions={() => useConditionsPanel.getState().open(selectedMarker.lat, selectedMarker.lng, selectedMarker.name)}
              onDelete={() => del.mutate(selectedMarker, { onSuccess: () => useSelection.getState().close() })}
              onClose={() => useSelection.getState().close()}
            />
          )}
          {markerPanel && sel.mode === 'edit' && selectedMarker && (
            <MarkerEditorPanel dark={dark} marker={selectedMarker} onClose={() => useSelection.getState().close()} onSaved={(m) => useSelection.getState().openDetail(m.id)} />
          )}
          {markerPanel && sel.mode === 'new' && sel.draft && (
            <MarkerEditorPanel
              key={`${sel.draft.lat},${sel.draft.lng}`}
              dark={dark}
              point={sel.draft}
              onClose={() => useSelection.getState().close()}
              onSaved={(m) => useSelection.getState().openDetail(m.id)}
            />
          )}
          {pathsPanel &&
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
                onClose={() => usePaths.getState().close()}
              />
            ) : selectedCollection ? (
              <CollectionDetailPanel
                dark={dark}
                collection={selectedCollection}
                resolveName={resolveItemName}
                onBack={() => usePaths.getState().openList()}
                onClose={() => usePaths.getState().close()}
              />
            ) : paths.tab === 'collections' ? (
              <CollectionsListPanel
                dark={dark}
                collections={collections}
                loading={collectionsQ.isLoading}
                onOpen={(id) => usePaths.getState().openCollection(id)}
                onClose={() => usePaths.getState().close()}
              />
            ) : (
              <PathsListPanel
                dark={dark}
                tracks={tracks}
                loading={tracksQ.isLoading}
                onSelect={(id) => usePaths.getState().openDetail(id)}
                onClose={() => usePaths.getState().close()}
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
            useUiStore.getState().closeAccount();
            useConditionsPanel.getState().close();
            useSelection.getState().close();
            usePaths.getState().close();
            useRouting.getState().open({ lat: ctxMenu.lat, lng: ctxMenu.lng });
          }}
          onForecast={(name) => useConditionsPanel.getState().open(ctxMenu.lat, ctxMenu.lng, name)}
          onClose={() => setCtxMenu(null)}
        />
      )}

      {toast && (
        <div
          style={{
            position: 'absolute',
            bottom: 24,
            left: '50%',
            transform: 'translateX(-50%)',
            zIndex: 40,
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
      )}

      {!ready && <div className="booting">Starting the map…</div>}
    </div>
    </MapEngineProvider>
  );
}
