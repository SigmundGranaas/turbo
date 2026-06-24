import type { Units } from './store/uiStore';

/** Distance for display, respecting the user's units. */
export function formatDistance(meters: number, units: Units): string {
  if (units === 'imperial') {
    const mi = meters / 1609.344;
    return mi >= 0.1 ? `${mi.toFixed(1)} mi` : `${Math.round(meters * 3.28084)} ft`;
  }
  return meters >= 1000 ? `${(meters / 1000).toFixed(1)} km` : `${Math.round(meters)} m`;
}

/** A vertical distance (ascent / elevation) for display. */
export function formatElev(meters: number, units: Units): string {
  return units === 'imperial' ? `${Math.round(meters * 3.28084)} ft` : `${Math.round(meters)} m`;
}

export function formatTemp(celsius: number, units: Units): string {
  return units === 'imperial' ? `${Math.round((celsius * 9) / 5 + 32)}°` : `${Math.round(celsius)}°`;
}

export function formatWind(ms: number, units: Units): string {
  return units === 'imperial' ? `${Math.round(ms * 2.23694)} mph` : `${Math.round(ms)} m/s`;
}
