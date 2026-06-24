import { useMemo, type CSSProperties, type ReactNode } from 'react';

/** The signature Turbo "Cookie" — a soft scalloped/lobed squircle used for
 *  avatars, brand badges, and list-row icon backings (carried over from the
 *  Android M3 Expressive app). Built as a clip-path so any child centers inside. */
function cookiePath(size: number, lobes: number): string {
  const c = size / 2;
  const steps = lobes * 16;
  const base = 0.92;
  const amp = 0.06; // peak ≈0.98·r, trough ≈0.86·r — gentle scallop
  let d = '';
  for (let i = 0; i <= steps; i++) {
    const t = (i / steps) * Math.PI * 2;
    const r = c * (base + amp * Math.cos(lobes * t));
    const x = c + r * Math.cos(t);
    const y = c + r * Math.sin(t);
    d += `${i ? 'L' : 'M'}${x.toFixed(2)} ${y.toFixed(2)}`;
  }
  return `${d}Z`;
}

export function Cookie({
  size = 40,
  lobes = 7,
  fill = 'var(--primary)',
  children,
  style,
}: {
  size?: number;
  lobes?: number;
  fill?: string;
  children?: ReactNode;
  style?: CSSProperties;
}) {
  const path = useMemo(() => cookiePath(size, lobes), [size, lobes]);
  return (
    <div
      style={{
        width: size,
        height: size,
        background: fill,
        clipPath: `path('${path}')`,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        flexShrink: 0,
        ...style,
      }}
    >
      {children}
    </div>
  );
}
