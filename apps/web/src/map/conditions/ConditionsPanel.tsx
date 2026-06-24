import { useState } from 'react';
import { windDir } from '../../api/conditions';
import { useConditions } from './useConditions';
import { useUiStore } from '../../store/uiStore';
import { formatTemp, formatWind } from '../../format';
import { SidePanel, Eyebrow, StatTile, Tabs } from '../../ui/Panel';
import { Icon } from '../../ui/Icon';

/** Full conditions panel: weather now + 24h outlook, and an ocean/tide tab.
 *  Ports the design's conditions sheet (avalanche tab deferred — needs a
 *  lat/lon→Varsom-region resolver the backend doesn't expose). */
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
