/** Activity / place kinds — the 18 kinds from the Android app, in the same
 *  order as the design's icon grid. `icon` is a Material Symbols name (also what
 *  we persist in the location's `display.icon`); `color` is the pin tint
 *  (client-side — the geo API has no colour field). See port docs 06 + 14. */
export interface ActivityKind {
  id: string;
  label: string;
  icon: string;
  color: string;
}

export const ACTIVITY_KINDS: ActivityKind[] = [
  { id: 'mountain', label: 'Mountain', icon: 'landscape', color: '#8f4c38' },
  { id: 'park', label: 'Park', icon: 'park', color: '#388E3C' },
  { id: 'beach', label: 'Beach', icon: 'beach_access', color: '#F57C00' },
  { id: 'forest', label: 'Forest', icon: 'forest', color: '#2E7D32' },
  { id: 'hiking', label: 'Hiking', icon: 'hiking', color: '#8f4c38' },
  { id: 'kayaking', label: 'Kayaking', icon: 'kayaking', color: '#1976D2' },
  { id: 'biking', label: 'Biking', icon: 'directions_bike', color: '#00897B' },
  { id: 'cabin', label: 'Cabin', icon: 'cabin', color: '#5D4037' },
  { id: 'parking', label: 'Parking', icon: 'local_parking', color: '#546E7A' },
  { id: 'camping', label: 'Camping', icon: 'airport_shuttle', color: '#6c5d2f' },
  { id: 'swimming', label: 'Swimming', icon: 'pool', color: '#0097A7' },
  { id: 'diving', label: 'Diving', icon: 'scuba_diving', color: '#00695C' },
  { id: 'viewpoint', label: 'Viewpoint', icon: 'photo_camera', color: '#7B1FA2' },
  { id: 'restaurant', label: 'Restaurant', icon: 'restaurant', color: '#C2185B' },
  { id: 'cafe', label: 'Café', icon: 'local_cafe', color: '#795548' },
  { id: 'accommodation', label: 'Accommodation', icon: 'hotel', color: '#5E35B1' },
  { id: 'fishing', label: 'Fishing', icon: 'phishing', color: '#0277BD' },
  { id: 'skiing', label: 'Skiing', icon: 'downhill_skiing', color: '#1565C0' },
];

const BY_ICON = new Map(ACTIVITY_KINDS.map((k) => [k.icon, k]));
export const DEFAULT_KIND = ACTIVITY_KINDS[4]; // hiking

/** Resolve a kind from a persisted icon name (falls back to a neutral default). */
export function kindForIcon(icon: string | undefined): ActivityKind {
  return (icon && BY_ICON.get(icon)) || DEFAULT_KIND;
}
