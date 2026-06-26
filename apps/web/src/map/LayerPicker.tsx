import { Glass } from '../ui/Glass';
import { Icon } from '../ui/Icon';
import { BASE_LAYERS, type BaseLayerId } from '../map-engine';

/** Base-layer chooser popover (opened by the MapRail "Map layers" button).
 *  Lists every entry in `BASE_LAYERS`; selecting one swaps the basemap in place
 *  (the engine re-ingests from the new source URL — see TurboMapCanvas). */
export function LayerPicker({
  dark,
  active,
  onSelect,
}: {
  dark: boolean;
  active: BaseLayerId;
  onSelect: (id: BaseLayerId) => void;
}) {
  const ids = Object.keys(BASE_LAYERS) as BaseLayerId[];
  return (
    <Glass dark={dark} radius={20} style={{ padding: 8, width: 200, display: 'flex', flexDirection: 'column', gap: 2 }}>
      <div style={{ font: '600 11px/14px var(--font-sans)', color: 'var(--on-surface-variant)', padding: '4px 8px 6px' }}>
        Base map
      </div>
      {ids.map((id) => {
        const def = BASE_LAYERS[id];
        const on = id === active;
        return (
          <button
            key={id}
            title={def.attribution}
            onClick={() => onSelect(id)}
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
              background: on ? 'var(--primary)' : 'transparent',
              color: on ? 'var(--on-primary)' : 'var(--on-surface)',
              font: '500 14px/18px var(--font-sans)',
            }}
          >
            <Icon name={def.icon} size={20} />
            <span style={{ flex: 1 }}>{def.label}</span>
            {on && <Icon name="check" size={18} />}
          </button>
        );
      })}
    </Glass>
  );
}
