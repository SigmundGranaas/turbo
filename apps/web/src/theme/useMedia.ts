import { useEffect, useState } from 'react';

/** True on phone-width viewports — drives the panel→bottom-sheet layout. */
export function useIsMobile(maxWidthPx = 640): boolean {
  const q = `(max-width: ${maxWidthPx}px)`;
  const [matches, setMatches] = useState(() => window.matchMedia(q).matches);
  useEffect(() => {
    const mq = window.matchMedia(q);
    const onChange = (e: MediaQueryListEvent) => setMatches(e.matches);
    mq.addEventListener('change', onChange);
    setMatches(mq.matches);
    return () => mq.removeEventListener('change', onChange);
  }, [q]);
  return matches;
}
