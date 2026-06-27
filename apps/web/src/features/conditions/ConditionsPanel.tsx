import { useState } from 'react';
import { windDir } from '../../api/conditions';
import { useConditions } from '../../api/useConditions';
import { useUiStore } from '../../store/uiStore';
import { formatTemp, formatWind } from '../../format';
import { SidePanel, Eyebrow, StatTile, Tabs } from '../../ui/Panel';
import { Icon } from '../../ui/Icon';

/** Compact emoji glyph for a met.no symbol_code (e.g. "partlycloudy_day",
 *  "lightrain"). Falls back to a neutral disc for unknown / absent codes. */
function symbolGlyph(code?: string): string {
  if (!code) return '·';
  const c = code.toLowerCase();
  if (c.includes('thunder')) return '⛈️';
  if (c.includes('sleet')) return '🌨️';
  if (c.includes('snow')) return '❄️';
  if (c.includes('rain')) return '🌧️';
  if (c.includes('fog')) return '🌫️';
  if (c.includes('cloudy') && !c.includes('partly')) return '☁️';
  if (c.includes('partlycloudy') || c.includes('fair')) return '⛅';
  if (c.includes('clearsky')) return '☀️';
  return '·';
}

/** Weekday label for an ISO `YYYY-MM-DD` (UTC) date — "Today" for the first day. */
function dayLabel(iso: string, isFirst: boolean): string {
  if (isFirst) return 'Today';
  const d = new Date(`${iso}T12:00:00Z`);
  return Number.isNaN(d.getTime()) ? iso : d.toLocaleDateString(undefined, { weekday: 'short' });
}

/** Full conditions panel: weather now + 24h outlook + multi-day forecast, and
 *  an ocean/tide tab. Ports the design's conditions sheet (avalanche tab
 *  deferred — needs a lat/lon→Varsom-region resolver the backend doesn't expose). */
export function ConditionsPanel({
  dark,
  lat,
  lng,
  name,
  onClose,
}: {
  dark: boolean;
  lat: number;
  lng: number;
  name: string;
  onClose: () => void;
}) {
  const units = useUiStore((s) => s.units);
  const cond = useConditions(lat, lng);
  const [tab, setTab] = useState(0);
  const now = cond.data?.now;
  const hourly = cond.data?.hourly ?? [];
  const daily = cond.data?.daily ?? [];
  const tide = cond.data?.tide;

  return (
    <SidePanel dark={dark} title="Conditions" subtitle={name} onClose={onClose}>
      <div style={{ paddingTop: 8, paddingBottom: 16 }}>
        <Tabs items={[{ label: 'Weather' }, { label: 'Ocean' }]} active={tab} onPick={setTab} />

        {cond.isLoading && <div style={{ marginTop: 16, font: '400 14px var(--font-sans)', color: 'var(--on-surface-variant)' }}>Loading…</div>}
        {cond.isError && <div style={{ marginTop: 16, font: '400 14px var(--font-sans)', color: 'var(--error)' }}>Weather is unavailable right now.</div>}

        {tab === 0 && now && (
          <>
            <div style={{ display: 'flex', alignItems: 'center', gap: 14, margin: '16px 0' }}>
              <div style={{ font: '300 56px/1 var(--font-sans)', color: 'var(--on-surface)', letterSpacing: -1 }}>{formatTemp(now.tempC, units)}</div>
              <div style={{ flex: 1 }}>
                <div style={{ font: '500 14px/18px var(--font-sans)', color: 'var(--on-surface)', textTransform: 'capitalize' }}>
                  {(now.symbol ?? '').replace(/_/g, ' ') || 'Current'}
                </div>
                <div style={{ font: '400 13px/17px var(--font-sans)', color: 'var(--on-surface-variant)' }}>
                  Wind {formatWind(now.windMs, units)} {windDir(now.windDeg)} · {Math.round(now.cloudPct)}% cloud
                </div>
              </div>
            </div>
            <div style={{ display: 'flex', gap: 10 }}>
              <StatTile icon="water_drop" value={(now.precipMm ?? 0).toFixed(1)} label="mm/h" />
              <StatTile icon="humidity_percentage" value={`${Math.round(now.humidityPct)}%`} label="Humidity" />
              <StatTile icon="filter_drama" value={`${Math.round(now.cloudPct)}%`} label="Cloud" />
            </div>

            <Eyebrow style={{ margin: '22px 0 10px' }}>Next 24 hours</Eyebrow>
            <div style={{ display: 'flex', flexDirection: 'column' }}>
              {hourly.map((h, i) => (
                <div key={i} style={{ display: 'flex', alignItems: 'center', gap: 12, padding: '8px 0', borderBottom: i < hourly.length - 1 ? '1px solid var(--outline-variant)' : 'none' }}>
                  <div style={{ width: 48, font: '500 13px/1 var(--font-sans)', color: 'var(--on-surface-variant)' }}>+{(i + 1) * 3}h</div>
                  <Icon name="water_drop" size={16} color="var(--on-surface-variant)" />
                  <div style={{ width: 44, font: '400 13px/1 var(--font-sans)', color: 'var(--on-surface-variant)' }}>{(h.precipMm ?? 0).toFixed(1)}</div>
                  <div style={{ flex: 1 }} />
                  <div style={{ font: '600 15px/1 var(--font-sans)', color: 'var(--on-surface)' }}>{formatTemp(h.tempC, units)}</div>
                </div>
              ))}
            </div>

            {daily.length > 0 && (
              <>
                <Eyebrow style={{ margin: '22px 0 10px' }}>Next days</Eyebrow>
                <div style={{ display: 'flex', flexDirection: 'column' }}>
                  {daily.map((d, i) => (
                    <div key={d.date} style={{ display: 'flex', alignItems: 'center', gap: 12, padding: '9px 0', borderBottom: i < daily.length - 1 ? '1px solid var(--outline-variant)' : 'none' }}>
                      <div style={{ width: 56, font: '500 13px/1 var(--font-sans)', color: 'var(--on-surface)' }}>{dayLabel(d.date, i === 0)}</div>
                      <div style={{ width: 22, fontSize: 16, textAlign: 'center' }} title={(d.symbol ?? '').replace(/_/g, ' ')}>{symbolGlyph(d.symbol)}</div>
                      <Icon name="water_drop" size={15} color="var(--on-surface-variant)" />
                      <div style={{ width: 38, font: '400 13px/1 var(--font-sans)', color: 'var(--on-surface-variant)' }}>{(d.precipMm ?? 0).toFixed(1)}</div>
                      <div style={{ flex: 1 }} />
                      <div style={{ font: '600 15px/1 var(--font-sans)', color: 'var(--on-surface)' }}>{formatTemp(d.highC, units)}</div>
                      <div style={{ width: 44, textAlign: 'right', font: '400 14px/1 var(--font-sans)', color: 'var(--on-surface-variant)' }}>{formatTemp(d.lowC, units)}</div>
                    </div>
                  ))}
                </div>
              </>
            )}
          </>
        )}

        {tab === 1 && (
          <div style={{ marginTop: 16 }}>
            {tide && (tide.heightM != null || tide.summary) ? (
              <>
                <div style={{ display: 'flex', gap: 10 }}>
                  <StatTile icon="waves" value={tide.heightM != null ? `${tide.heightM.toFixed(1)} m` : '—'} label="Tide height" />
                </div>
                {tide.summary && (
                  <div style={{ marginTop: 14, font: '400 14px/21px var(--font-sans)', color: 'var(--on-surface-variant)' }}>{tide.summary}</div>
                )}
              </>
            ) : (
              <div style={{ font: '400 14px/21px var(--font-sans)', color: 'var(--on-surface-variant)' }}>
                No marine data here — this point looks to be inland.
              </div>
            )}
          </div>
        )}
      </div>
    </SidePanel>
  );
}
