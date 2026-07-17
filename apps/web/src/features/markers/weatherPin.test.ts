import { describe, it, expect } from 'vitest';
import {
  WEATHER_PIN_STALE_MS,
  WEATHER_PIN_ICON_PREFIX,
  weatherPinFetchDecision,
  weatherPinUiState,
  encodeWireIcon,
  decodeWireIcon,
  type WeatherSnapshot,
} from './weatherPin';

const snap = (over: Partial<WeatherSnapshot> = {}): WeatherSnapshot => ({
  temperatureC: 7,
  symbolCode: 'lightrain',
  windSpeedMs: 4,
  windFromDeg: 270,
  precipitationMm: 0.4,
  ...over,
});

/** When a weather pin opens/drops it decides whether to refresh from MET or
 *  render its cached forecast — the behaviour that keeps it live yet
 *  offline-safe. */
describe('weatherPinFetchDecision', () => {
  const HOUR = WEATHER_PIN_STALE_MS;

  it('never fetches offline — even with no cache, a fetch cannot succeed', () => {
    expect(weatherPinFetchDecision(null, false)).toBe('UseCache');
    expect(weatherPinFetchDecision(5 * HOUR, false)).toBe('UseCache');
  });

  it('fetches online when there is no cached forecast', () => {
    expect(weatherPinFetchDecision(null, true)).toBe('Fetch');
  });

  it('fetches online when the cache is older than an hour', () => {
    expect(weatherPinFetchDecision(HOUR + 1, true)).toBe('Fetch');
  });

  it('keeps the cache online when it is still fresh', () => {
    expect(weatherPinFetchDecision(HOUR - 1, true)).toBe('UseCache');
    expect(weatherPinFetchDecision(HOUR, true)).toBe('UseCache');
    expect(weatherPinFetchDecision(0, true)).toBe('UseCache');
  });

  it('honours a custom staleness window', () => {
    expect(weatherPinFetchDecision(10_000, true, 5_000)).toBe('Fetch');
    expect(weatherPinFetchDecision(4_000, true, 5_000)).toBe('UseCache');
  });
});

/** What the on-map glyph and its expanded card show, derived from the cached
 *  forecast — including how long ago it was updated and whether marine data is
 *  available. */
describe('weatherPinUiState', () => {
  const NOW = 1_000_000_000_000;

  it('is null when the pin has no cached forecast yet', () => {
    expect(weatherPinUiState({}, NOW)).toBeNull();
    expect(weatherPinUiState({ forecastFetchedAtEpochMs: NOW }, NOW)).toBeNull();
  });

  it('surfaces temperature, condition glyph code and the expanded air fields', () => {
    const s = weatherPinUiState(
      { forecast: snap(), forecastFetchedAtEpochMs: NOW },
      NOW,
    )!;
    expect(s.temperatureC).toBe(7);
    expect(s.symbolCode).toBe('lightrain');
    expect(s.expanded.windSpeedMs).toBe(4);
    expect(s.expanded.windFromDeg).toBe(270);
    expect(s.expanded.precipitationMm).toBe(0.4);
  });

  it('reports whole hours since the cache was written', () => {
    const threeHalfHoursAgo = NOW - 3.5 * WEATHER_PIN_STALE_MS;
    const s = weatherPinUiState({ forecast: snap(), forecastFetchedAtEpochMs: threeHalfHoursAgo }, NOW)!;
    expect(s.updatedHoursAgo).toBe(3);
  });

  it('has a null updated age when the cache carries no timestamp', () => {
    const s = weatherPinUiState({ forecast: snap() }, NOW)!;
    expect(s.updatedHoursAgo).toBeNull();
  });

  it('flags marine data only when wave/sea-temperature is present', () => {
    const inland = weatherPinUiState({ forecast: snap(), forecastFetchedAtEpochMs: NOW }, NOW)!;
    expect(inland.hasMarine).toBe(false);

    const atSea = weatherPinUiState(
      { forecast: snap({ waveHeightM: 1.2, waveFromDeg: 200, seaTemperatureC: 9 }), forecastFetchedAtEpochMs: NOW },
      NOW,
    )!;
    expect(atSea.hasMarine).toBe(true);
    expect(atSea.expanded.waveHeightM).toBe(1.2);
    expect(atSea.expanded.seaTemperatureC).toBe(9);
  });
});

/** The marker kind rides on the wire `icon` (the geo location has no kind
 *  field), so a weather pin must survive a sync round-trip instead of coming
 *  back as a plain marker — the latent data-loss fix. */
describe('wire-icon codec', () => {
  it('namespaces a weather pin, leaving standard markers untouched', () => {
    expect(encodeWireIcon('hiking', 'WeatherPin')).toBe(`${WEATHER_PIN_ICON_PREFIX}hiking`);
    expect(encodeWireIcon('hiking', 'Standard')).toBe('hiking');
  });

  it('decodes the prefix back to a weather pin and its activity icon', () => {
    expect(decodeWireIcon('weatherpin:kayaking')).toEqual({ activityIcon: 'kayaking', markerKind: 'WeatherPin' });
    expect(decodeWireIcon('camping')).toEqual({ activityIcon: 'camping', markerKind: 'Standard' });
  });

  it('defaults a blank icon to a standard mountain pin', () => {
    expect(decodeWireIcon('')).toEqual({ activityIcon: 'mountain', markerKind: 'Standard' });
  });

  it('round-trips both kinds through encode → decode', () => {
    for (const [icon, kind] of [
      ['hiking', 'Standard'],
      ['kayaking', 'WeatherPin'],
      ['fishing', 'WeatherPin'],
    ] as const) {
      expect(decodeWireIcon(encodeWireIcon(icon, kind))).toEqual({ activityIcon: icon, markerKind: kind });
    }
  });
});
