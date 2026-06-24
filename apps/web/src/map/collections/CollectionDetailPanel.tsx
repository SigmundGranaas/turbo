import { useState } from 'react';
import type { Collection, CollectionItem } from '../../api/collections';
import { useCollectionMutations } from './useCollections';
import { SidePanel, Eyebrow, FilledField } from '../../ui/Panel';
import { Btn } from '../../ui/Glass';
import { Icon } from '../../ui/Icon';

const SWATCHES = ['#8f4c38', '#1976D2', '#388E3C', '#F57C00', '#7B1FA2', '#00897B', '#C2185B'];

/** Collection detail — rename, recolour, member list (remove), delete. */
export function CollectionDetailPanel({
  dark,
  collection,
  resolveName,
  onBack,
  onClose,
}: {
  dark: boolean;
  collection: Collection;
  resolveName: (item: CollectionItem) => string;
  onBack: () => void;
  onClose: () => void;
}) {
  const { update, remove, removeItem } = useCollectionMutations();
  const [name, setName] = useState(collection.name);

  const saveName = () => {
    if (name.trim() && name.trim() !== collection.name) update.mutate({ c: collection, changes: { name: name.trim() } });
  };

  return (
    <SidePanel
      dark={dark}
      title={collection.name}
      subtitle={`${collection.items.length} item${collection.items.length === 1 ? '' : 's'}`}
      onClose={onClose}
      headerExtra={
        <button onClick={onBack} title="Back" style={{ width: 38, height: 38, borderRadius: 9999, border: 'none', cursor: 'pointer', background: 'var(--surface-container-highest)', color: 'var(--on-surface-variant)', display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0 }}>
          <Icon name="arrow_back" size={20} color="var(--on-surface-variant)" />
        </button>
      }
    >
      <div style={{ paddingTop: 8, paddingBottom: 16 }}>
        <FilledField label="Name" value={name} onChange={setName} />
        <div style={{ display: 'flex', justifyContent: 'flex-end', marginTop: 8 }}>
          <Btn label="Rename" tone="surface" size="sm" onClick={saveName} />
        </div>

        <Eyebrow style={{ margin: '18px 0 10px' }}>Colour</Eyebrow>
        <div style={{ display: 'flex', gap: 12 }}>
          {SWATCHES.map((c) => (
            <button
              key={c}
              onClick={() => update.mutate({ c: collection, changes: { colorHex: c } })}
              style={{ width: 32, height: 32, borderRadius: '50%', background: c, cursor: 'pointer', border: 'none', boxShadow: collection.colorHex === c ? `0 0 0 3px var(--surface), 0 0 0 5px ${c}` : 'none' }}
            />
          ))}
        </div>

        <Eyebrow style={{ margin: '20px 0 8px' }}>Items</Eyebrow>
        {collection.items.length === 0 && (
          <div style={{ font: '400 13px/19px var(--font-sans)', color: 'var(--on-surface-variant)' }}>
            Empty. Add markers and paths from their detail panel.
          </div>
        )}
        <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
          {collection.items.map((item) => (
            <div key={`${item.type}-${item.uuid}`} style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
              <Icon name={item.type === 'marker' ? 'place' : 'route'} size={20} color="var(--primary)" weight={500} />
              <div style={{ flex: 1, minWidth: 0, font: '500 14px/18px var(--font-sans)', color: 'var(--on-surface)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                {resolveName(item)}
              </div>
              <button onClick={() => removeItem.mutate({ c: collection, item })} title="Remove" style={{ border: 'none', background: 'transparent', cursor: 'pointer', color: 'var(--on-surface-variant)', display: 'flex' }}>
                <Icon name="close" size={18} color="var(--on-surface-variant)" />
              </button>
            </div>
          ))}
        </div>

        <div style={{ marginTop: 22 }}>
          <Btn label="Delete collection" icon="delete" tone="surface" full onClick={() => remove.mutate(collection, { onSuccess: onBack })} />
        </div>
      </div>
    </SidePanel>
  );
}
