import { useEffect } from 'react';
import { setMapContent } from './mapContent';

/** The blue "you are here" dot — scene-declared map content (plan P6.3).
 *  Watches browser geolocation and publishes the latest fix to the content
 *  plane; the engine draws the accuracy halo + white-ringed dot as `circle`
 *  layers, draped on the terrain like everything else. No DOM: this
 *  component renders nothing. */
export function UserLocationLayer() {
  useEffect(() => {
    if (!('geolocation' in navigator)) return;
    const id = navigator.geolocation.watchPosition(
      (pos) => {
        setMapContent({ userFix: { lat: pos.coords.latitude, lng: pos.coords.longitude } });
      },
      () => {},
      { enableHighAccuracy: true, maximumAge: 5000, timeout: 15000 },
    );
    return () => {
      navigator.geolocation.clearWatch(id);
      setMapContent({ userFix: null });
    };
  }, []);

  return null;
}
