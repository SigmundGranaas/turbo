import type { CSSProperties } from 'react';

/** A Material Symbols (Outlined) glyph — the icon primitive used across the kit.
 *  `fill`/`weight`/`size` map to the variable-font axes. Color defaults to
 *  currentColor so it inherits unless overridden. */
export function Icon({
  name,
  size = 24,
  fill = false,
  weight = 400,
  color,
  style,
}: {
  name: string;
  size?: number;
  fill?: boolean;
  weight?: number;
  color?: string;
  style?: CSSProperties;
}) {
  return (
    <span
      className="material-symbols-outlined"
      style={{
        fontSize: size,
        color,
        fontVariationSettings: `'opsz' ${size}, 'wght' ${weight}, 'FILL' ${fill ? 1 : 0}, 'GRAD' 0`,
        ...style,
      }}
    >
      {name}
    </span>
  );
}
