import { useEffect, useState } from 'react';
import { useUiStore } from '../store/uiStore';

const mql = () => window.matchMedia('(prefers-color-scheme: dark)');

/** Resolve the effective dark/light from the theme preference. `system` tracks
 *  the OS scheme live; explicit light/dark win. */
export function useResolvedDark(): boolean {
  const theme = useUiStore((s) => s.theme);
  const [sysDark, setSysDark] = useState(() => mql().matches);
  useEffect(() => {
    if (theme !== 'system') return;
    const mq = mql();
    const onChange = (e: MediaQueryListEvent) => setSysDark(e.matches);
    mq.addEventListener('change', onChange);
    setSysDark(mq.matches);
    return () => mq.removeEventListener('change', onChange);
  }, [theme]);
  return theme === 'system' ? sysDark : theme === 'dark';
}
