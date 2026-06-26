import { MapScreen } from './map/MapScreen';
import { MapEngineProvider } from './map-core';

export default function App() {
  // The map-engine context wraps the whole map host, so the host body (not just
  // its rendered overlays) can read the live engine via useMapEngine().
  return (
    <MapEngineProvider>
      <MapScreen />
    </MapEngineProvider>
  );
}
