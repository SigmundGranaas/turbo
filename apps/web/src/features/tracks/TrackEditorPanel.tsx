import { useState } from 'react';
import type { Track } from './api';
import { ACTIVITY_KINDS, kindForIcon } from '../../activities/kinds';
import { SidePanel, Eyebrow, FilledField } from '../../ui/Panel';
import { Btn } from '../../ui/Glass';
import { Icon } from '../../ui/Icon';
import { useUpdateTrack } from './useTracks';

/** A track's display colours — a small fixed palette (line colour on the map +
 *  swatch in lists). Mirrors the marker activity palette in spirit. */
const TRACK_COLORS = ['#C75B39', '#2563EB', '#059669', '#7C3AED', '#DB2777', '#D97706', '#0891B2', '#475569'];

/** Line styles a track can render with. Keys are the shared cross-client
 *  vocabulary (synced as `lineStyleKey`; see `dashArrayFor` for the map
 *  pattern); `preview` is only the button's SVG dash sample. */
const LINE_STYLES = [
  { key: 'solid', label: 'Solid', preview: undefined },
  { key: 'dotted', label: 'Dotted', preview: '0.5 6' },
  { key: 'dashed', label: 'Dashed', preview: '8 5' },
  { key: 'dash_dot', label: 'Dash-dot', preview: '8 5 0.5 5' },
];

/** Edit a saved track's display metadata: name, note, icon, and line colour.
 *  Mirrors `MarkerEditorPanel`; saves via the tracks `PUT` changeset. */
export function TrackEditorPanel({
  dark,
  track,
  onClose,
  onSaved,
}: {
  dark: boolean;
  track: Track;
  onClose: () => void;
  onSaved: (t: Track) => void;
}) {
  const [name, setName] = useState(track.name ?? '');
  const [note, setNote] = useState(track.description ?? '');
  const [iconName, setIconName] = useState(kindForIcon(track.iconKey).icon);
  const [color, setColor] = useState(track.colorHex || TRACK_COLORS[0]);
  const [lineStyle, setLineStyle] = useState(track.lineStyleKey || 'solid');

  const update = useUpdateTrack();
  const busy = update.isPending;

  const save = () => {
    update.mutate(
      {
        track,
        changes: {
          name: name.trim() || kindForIcon(iconName).label,
          description: note,
          iconKey: iconName,
          colorHex: color,
          lineStyleKey: lineStyle,
        },
      },
      { onSuccess: onSaved },
    );
  };

  return (
    <SidePanel
      dark={dark}
      title="Edit path"
      subtitle={track.name || 'Untitled path'}
      onClose={onClose}
      footer={
        <div style={{ display: 'flex', gap: 10 }}>
          <Btn label="Cancel" tone="surface" full onClick={onClose} />
          <Btn label={busy ? 'Saving…' : 'Save'} icon="check" full onClick={busy ? undefined : save} />
        </div>
      }
    >
      <div style={{ paddingTop: 8 }}>
        <FilledField label="Name" value={name} onChange={setName} placeholder="Name this path" />

        <Eyebrow style={{ margin: '22px 0 10px' }}>Colour</Eyebrow>
        <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap' }}>
          {TRACK_COLORS.map((c) => {
            const sel = c.toLowerCase() === color.toLowerCase();
            return (
              <button
                key={c}
                title={c}
                onClick={() => setColor(c)}
                style={{
                  width: 34,
                  height: 34,
                  borderRadius: '50%',
                  cursor: 'pointer',
                  background: c,
                  border: sel ? '3px solid var(--on-surface)' : '3px solid transparent',
                  boxShadow: '0 1px 3px rgba(0,0,0,.25)',
                }}
              />
            );
          })}
        </div>

        <Eyebrow style={{ margin: '22px 0 10px' }}>Line style</Eyebrow>
        <div style={{ display: 'flex', gap: 8 }}>
          {LINE_STYLES.map((s) => {
            const sel = s.key === lineStyle;
            return (
              <button
                key={s.key}
                title={s.label}
                onClick={() => setLineStyle(s.key)}
                style={{
                  flex: 1,
                  height: 40,
                  borderRadius: sel ? 14 : 10,
                  cursor: 'pointer',
                  border: 'none',
                  background: sel ? 'var(--primary)' : 'var(--surface-container-high)',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                }}
              >
                <svg width="44" height="8" aria-label={s.label}>
                  <line
                    x1="2" y1="4" x2="42" y2="4"
                    stroke={sel ? 'var(--on-primary)' : 'var(--primary)'}
                    strokeWidth="3.5"
                    strokeLinecap="round"
                    strokeDasharray={s.preview}
                  />
                </svg>
              </button>
            );
          })}
        </div>

        <Eyebrow style={{ margin: '22px 0 10px' }}>Icon</Eyebrow>
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(6, 1fr)', gap: 8 }}>
          {ACTIVITY_KINDS.map((k) => {
            const sel = k.icon === iconName;
            return (
              <button
                key={k.id}
                title={k.label}
                onClick={() => setIconName(k.icon)}
                style={{
                  aspectRatio: '1',
                  borderRadius: sel ? 16 : 12,
                  cursor: 'pointer',
                  border: 'none',
                  background: sel ? 'var(--primary)' : 'var(--surface-container-high)',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                }}
              >
                <Icon name={k.icon} size={20} color={sel ? 'var(--on-primary)' : 'var(--primary)'} fill={sel} weight={500} />
              </button>
            );
          })}
        </div>

        <Eyebrow style={{ margin: '22px 0 10px' }}>Note</Eyebrow>
        <FilledField label="Note" value={note} onChange={setNote} placeholder="Add a note (optional)" multiline />

        {update.error && (
          <div style={{ marginTop: 14, font: '400 13px/18px var(--font-sans)', color: 'var(--error)' }}>
            Couldn’t save — the path may have changed elsewhere. Reopen and try again.
          </div>
        )}
        <div style={{ height: 16 }} />
      </div>
    </SidePanel>
  );
}
