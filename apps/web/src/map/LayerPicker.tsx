import { useState } from 'react';
import { Glass } from '../ui/Glass';
import { Icon } from '../ui/Icon';
import { BASE_LAYERS, type BaseLayerId } from '../map-engine';
import { isValidXyzTemplate, type CustomBaseLayer } from '../baseLayers';
import { DEFAULT_3D_DETENT, MAX_3D_EXAGGERATION } from '../map-core';
import { useUiStore } from '../store/uiStore';

/** Base-layer chooser popover (opened by the MapRail "Map layers" button).
 *  Lists every entry in `BASE_LAYERS` plus the user's own custom XYZ sources;
 *  selecting one swaps the basemap in place (the engine re-ingests from the new
 *  source URL — see TurboMapCanvas). An inline "Add map…" form takes a name +
 *  `{z}/{x}/{y}` template (same validation as the Android sheet). */
export function LayerPicker({
  dark,
  active,
  onSelect,
  threeDLevel,
  onThreeDLevel,
  sunLevel,
  onSunLevel,
}: {
  dark: boolean;
  active: BaseLayerId;
  onSelect: (id: BaseLayerId) => void;
  /** 3D-terrain slider level `[0, MAX_3D_EXAGGERATION]`; 0 = flat 2D (tilt locked). */
  threeDLevel: number;
  onThreeDLevel: (v: number) => void;
  /** Sun slider level `[0, 1]`; 0 = off. Lights the DEM relief in 2D and 3D. */
  sunLevel: number;
  onSunLevel: (v: number) => void;
}) {
  const customLayers = useUiStore((s) => s.customLayers);
  const [adding, setAdding] = useState(false);
  const [name, setName] = useState('');
  const [url, setUrl] = useState('');
  const urlValid = isValidXyzTemplate(url);

  const ids = Object.keys(BASE_LAYERS) as BaseLayerId[];
  const row = (id: BaseLayerId, label: string, icon: string, title: string, custom?: CustomBaseLayer) => {
    const on = id === active;
    return (
      <div key={id} style={{ display: 'flex', alignItems: 'center', gap: 2 }}>
        <button
          title={title}
          onClick={() => onSelect(id)}
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: 10,
            flex: 1,
            minWidth: 0,
            padding: '9px 10px',
            borderRadius: 12,
            border: 'none',
            cursor: 'pointer',
            textAlign: 'left',
            background: on ? 'var(--primary)' : 'transparent',
            color: on ? 'var(--on-primary)' : 'var(--on-surface)',
            font: '500 14px/18px var(--font-sans)',
          }}
        >
          <Icon name={icon} size={20} />
          <span style={{ flex: 1, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{label}</span>
          {on && <Icon name="check" size={18} />}
        </button>
        {custom && (
          <button
            title="Remove custom map"
            onClick={() => useUiStore.getState().removeCustomLayer(custom.id)}
            style={{ border: 'none', background: 'transparent', cursor: 'pointer', color: 'var(--on-surface-variant)', padding: 4 }}
          >
            <Icon name="close" size={16} />
          </button>
        )}
      </div>
    );
  };

  const addLayer = () => {
    if (!urlValid) return;
    useUiStore.getState().addCustomLayer({
      id: `custom-${crypto.randomUUID()}`,
      label: name.trim() || 'Custom map',
      url: url.trim(),
      maxZoom: 19,
    });
    setAdding(false);
    setName('');
    setUrl('');
  };

  return (
    <Glass dark={dark} radius={20} style={{ padding: 8, width: 232, display: 'flex', flexDirection: 'column', gap: 2 }}>
      <div style={{ font: '600 11px/14px var(--font-sans)', color: 'var(--on-surface-variant)', padding: '4px 8px 6px' }}>
        Base map
      </div>
      {ids.map((id) => {
        const def = BASE_LAYERS[id];
        return row(id, def.label, def.icon, def.attribution);
      })}
      {customLayers.map((c) => row(c.id, c.label, 'public', c.url, c))}
      {adding ? (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 6, padding: '6px 8px' }}>
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="Name"
            style={{ padding: '7px 9px', borderRadius: 10, border: '1px solid var(--outline)', background: 'var(--surface)', color: 'var(--on-surface)', font: '400 13px/16px var(--font-sans)' }}
          />
          <input
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            placeholder="https://…/{z}/{x}/{y}.png"
            style={{ padding: '7px 9px', borderRadius: 10, border: `1px solid ${url && !urlValid ? 'var(--error, #B3261E)' : 'var(--outline)'}`, background: 'var(--surface)', color: 'var(--on-surface)', font: '400 13px/16px var(--font-sans)' }}
          />
          {url && !urlValid && (
            <div style={{ font: '400 11px/14px var(--font-sans)', color: 'var(--error, #B3261E)' }}>
              Needs http(s) and {'{z}/{x}/{y}'} placeholders
            </div>
          )}
          <div style={{ display: 'flex', gap: 6 }}>
            <button
              onClick={() => setAdding(false)}
              style={{ flex: 1, padding: '7px 0', borderRadius: 10, border: 'none', cursor: 'pointer', background: 'transparent', color: 'var(--on-surface-variant)', font: '600 13px/16px var(--font-sans)' }}
            >
              Cancel
            </button>
            <button
              onClick={addLayer}
              disabled={!urlValid}
              style={{ flex: 1, padding: '7px 0', borderRadius: 10, border: 'none', cursor: urlValid ? 'pointer' : 'default', background: urlValid ? 'var(--primary)' : 'var(--surface-container-high)', color: urlValid ? 'var(--on-primary)' : 'var(--on-surface-variant)', font: '600 13px/16px var(--font-sans)' }}
            >
              Add
            </button>
          </div>
        </div>
      ) : (
        <button
          onClick={() => setAdding(true)}
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: 10,
            width: '100%',
            padding: '9px 10px',
            borderRadius: 12,
            border: 'none',
            cursor: 'pointer',
            textAlign: 'left',
            background: 'transparent',
            color: 'var(--primary)',
            font: '600 14px/18px var(--font-sans)',
          }}
        >
          <Icon name="add" size={20} />
          <span style={{ flex: 1 }}>Add map…</span>
        </button>
      )}

      {/* Terrain & light — the relocated 3D + Sun controls (Phase 1). Both are
          pure sliders that never move the camera (the hard decoupling rule): 3D
          unlocks tilt gestures + exaggerates relief; Sun lights the relief
          top-down and works in 2D too. */}
      <div style={{ font: '600 11px/14px var(--font-sans)', color: 'var(--on-surface-variant)', padding: '10px 8px 4px' }}>
        Terrain &amp; light
      </div>
      <LayerSlider
        icon="terrain"
        title="3D terrain"
        subtitle="Tilt to explore the relief"
        value={threeDLevel}
        min={0}
        max={MAX_3D_EXAGGERATION}
        step={0.5}
        defaultOn={DEFAULT_3D_DETENT}
        onChange={onThreeDLevel}
      />
      <LayerSlider
        icon="wb_sunny"
        title="Sun"
        subtitle="Light &amp; shade the terrain"
        value={sunLevel}
        min={0}
        max={1}
        step={0.01}
        defaultOn={0.5}
        onChange={onSunLevel}
      />
    </Glass>
  );
}

/** A layer row with an enable switch and, when on, a fine-tune slider — the
 *  3D-terrain + Sun controls. The switch snaps the value between 0 (off) and
 *  [defaultOn] (a sensible detent), so enabling doesn't force a drag from zero;
 *  the slider then tunes it. Mirrors the Android `LayerSlider`. Neither control
 *  ever moves the camera — that decoupling lives in the environment reducer. */
function LayerSlider({
  icon,
  title,
  subtitle,
  value,
  min,
  max,
  step,
  defaultOn,
  onChange,
}: {
  icon: string;
  title: string;
  subtitle: string;
  value: number;
  min: number;
  max: number;
  step: number;
  defaultOn: number;
  onChange: (v: number) => void;
}) {
  const on = value > 0;
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 8, padding: '6px 8px 2px' }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
        <Icon name={icon} size={20} />
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{ font: '500 14px/18px var(--font-sans)', color: 'var(--on-surface)' }}>{title}</div>
          <div style={{ font: '400 11px/14px var(--font-sans)', color: 'var(--on-surface-variant)' }}>{subtitle}</div>
        </div>
        <button
          role="switch"
          aria-checked={on}
          aria-label={title}
          title={title}
          onClick={() => onChange(on ? 0 : defaultOn)}
          style={{
            position: 'relative',
            width: 40,
            height: 24,
            borderRadius: 12,
            border: 'none',
            cursor: 'pointer',
            flexShrink: 0,
            background: on ? 'var(--primary)' : 'var(--surface-container-high)',
            transition: 'background .15s',
          }}
        >
          <span
            style={{
              position: 'absolute',
              top: 3,
              left: on ? 19 : 3,
              width: 18,
              height: 18,
              borderRadius: 9,
              background: on ? 'var(--on-primary)' : 'var(--outline)',
              transition: 'left .15s',
            }}
          />
        </button>
      </div>
      {on && (
        <input
          type="range"
          min={min}
          max={max}
          step={step}
          value={value}
          onChange={(e) => onChange(parseFloat(e.target.value))}
          aria-label={`${title} amount`}
          style={{ width: '100%', accentColor: 'var(--primary)', cursor: 'pointer', paddingLeft: 30 }}
        />
      )}
    </div>
  );
}
