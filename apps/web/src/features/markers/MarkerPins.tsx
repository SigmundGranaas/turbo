import { useEffect } from 'react';
import { setMapContent } from '../../map-core';
import type { Marker } from './api';
import { kindForIcon } from '../../activities/kinds';

/** Marker pins as scene-declared map content (plan P6.3): this component
 *  publishes the pin set (kind-tinted, selection-emphasized) to the content
 *  plane and renders nothing — the engine draws the pins as `circle` layers
 *  in the one Scene document, and taps resolve through the engine's
 *  hit-testing in the host (`MapScreen.onMapTap`). The tap-opened detail
 *  panel stays DOM, as interactive chrome should. */
export function MarkerPins({
  markers,
  selectedId,
}: {
  markers: Marker[];
  selectedId?: string;
}) {
  useEffect(() => {
    setMapContent({
      pins: markers.map((mk) => ({
        id: mk.id,
        lat: mk.lat,
        lng: mk.lng,
        color: kindForIcon(mk.icon).color,
      })),
      selectedPinId: selectedId,
    });
  }, [markers, selectedId]);
  useEffect(() => () => setMapContent({ pins: [], selectedPinId: undefined }), []);

  return null;
}
