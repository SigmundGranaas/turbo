import { useState } from 'react';
import type { Marker } from '../../api/markers';
import { ACTIVITY_KINDS, DEFAULT_KIND, kindForIcon } from '../../activities/kinds';
import { SidePanel, Eyebrow, FilledField } from '../../ui/Panel';
import { Btn } from '../../ui/Glass';
import { Icon } from '../../ui/Icon';
import { useCreateMarker, useUpdateMarker } from './useMarkers';

const fmt = (lat: number, lng: number) =>
  `${Math.abs(lat).toFixed(4)}° ${lat >= 0 ? 'N' : 'S'}, ${Math.abs(lng).toFixed(4)}° ${lng >= 0 ? 'E' : 'W'}`;

/** Create or edit a marker. New mode is opened from a long-press/right-click on
 *  the map (point + reverse-geocoded name prefill); edit mode from the detail
 *  panel. Ports the design's `NewMarker` editor (name field, icon grid, note). */
export function MarkerEditorPanel({
  dark,
  marker,
  point,
  onClose,
  onSaved,
}: {
  dark: boolean;
  marker?: Marker;
  point?: { lat: number; lng: number; name: string };
  onClose: () => void;
  onSaved: (m: Marker) => void;
}) {
  const editing = Boolean(marker);
  const lat = marker?.lat ?? point?.lat ?? 0;
  const lng = marker?.lng ?? point?.lng ?? 0;

  const [name, setName] = useState(marker?.name ?? point?.name ?? '');
  const [note, setNote] = useState(marker?.description ?? '');
  const [iconName, setIconName] = useState((marker ? kindForIcon(marker.icon) : DEFAULT_KIND).icon);

  const create = useCreateMarker();
  const update = useUpdateMarker();
  const busy = create.isPending || update.isPending;
  const error = create.error || update.error;

  const save = () => {
    const finalName = name.trim() || kindForIcon(iconName).label;
    if (editing && marker) {
      update.mutate(
        { ...marker, name: finalName, description: note, icon: iconName },
        { onSuccess: onSaved },
      );
    } else {
      create.mutate({ lat, lng, name: finalName, icon: iconName, description: note }, { onSuccess: onSaved });
    }
  };

  return (
    <SidePanel
      dark={dark}
      title={editing ? 'Edit marker' : 'New marker'}
      subtitle={fmt(lat, lng)}
      onClose={onClose}
      footer={
        <div style={{ display: 'flex', gap: 10 }}>
          <Btn label="Cancel" tone="surface" full onClick={onClose} />
          <Btn label={busy ? 'Saving…' : 'Save'} icon="check" full onClick={busy ? undefined : save} />
        </div>
      }
    >
      <div style={{ paddingTop: 8 }}>
        <FilledField label="Name" value={name} onChange={setName} placeholder="Name this place" />

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

        {error && (
          <div style={{ marginTop: 14, font: '400 13px/18px var(--font-sans)', color: 'var(--error)' }}>
            Couldn’t save — sign in to save markers to your account.
          </div>
        )}
        <div style={{ height: 16 }} />
      </div>
    </SidePanel>
  );
}
