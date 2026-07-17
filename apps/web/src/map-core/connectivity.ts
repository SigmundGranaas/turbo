import { useSyncExternalStore } from 'react';

/** Live browser connectivity — `navigator.onLine`, kept current via the
 *  `online`/`offline` events. The map-point card reads this to gate the Measure
 *  action (see `measureAvailability`). SSR-safe (assumes online on the server). */
function subscribe(onChange: () => void): () => void {
  window.addEventListener('online', onChange);
  window.addEventListener('offline', onChange);
  return () => {
    window.removeEventListener('online', onChange);
    window.removeEventListener('offline', onChange);
  };
}

export function useOnline(): boolean {
  return useSyncExternalStore(
    subscribe,
    () => navigator.onLine,
    () => true,
  );
}
