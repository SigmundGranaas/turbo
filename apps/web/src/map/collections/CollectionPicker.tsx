import { useState } from 'react';
import type { CollectionItem } from '../../api/collections';
import { useCollections, useCollectionMutations } from './useCollections';
import { Glass } from '../../ui/Glass';
import { Icon } from '../../ui/Icon';

/** Centered "add to collection" picker — toggle the target marker/path into any
 *  collection, or create a new one. Opened from a detail panel's bookmark. */
export function CollectionPicker({ dark, item, onClose }: { dark: boolean; item: CollectionItem; onClose: () => void }) {
  const { data, isLoading } = useCollections();
  const { create, addItem, removeItem } = useCollectionMutations();
  const collections = data ?? [];
  const [name, setName] = useState('');

  const has = (uuids: CollectionItem[]) => uuids.some((i) => i.type === item.type && i.uuid === item.uuid);

  return (
    <div
      onClick={onClose}
      style={{ position: 'fixed', inset: 0, zIndex: 30, background: 'rgba(20,12,8,.45)', display: 'grid', placeItems: 'center' }}
    >
      <Glass dark={dark} level="panel" radius={24} style={{ width: 380, maxHeight: '70vh', display: 'flex', flexDirection: 'column', overflow: 'hidden' }} onClick={() => undefined}>
        <div onClick={(e) => e.stopPropagation()} style={{ display: 'flex', flexDirection: 'column', minHeight: 0 }}>
          <div style={{ padding: '18px 20px 8px', font: '700 18px/22px var(--font-sans)', color: 'var(--on-surface)' }}>Add to collection</div>
          <div style={{ flex: 1, overflowY: 'auto', padding: '0 12px' }}>
            {isLoading && <div style={{ padding: 12, color: 'var(--on-surface-variant)', font: '400 14px var(--font-sans)' }}>Loading…</div>}
            {collections.map((c) => {
              const inIt = has(c.items);
              return (
                <button
                  key={c.id}
                  onClick={() => (inIt ? removeItem.mutate({ c, item }) : addItem.mutate({ c, item }))}
                  style={{ width: '100%', display: 'flex', alignItems: 'center', gap: 12, padding: '10px 12px', border: 'none', background: 'transparent', cursor: 'pointer', textAlign: 'left', borderRadius: 12 }}
                >
                  <div style={{ width: 28, height: 28, borderRadius: 9, background: c.colorHex ?? 'var(--primary)', flexShrink: 0 }} />
                  <div style={{ flex: 1, minWidth: 0, font: '500 15px/20px var(--font-sans)', color: 'var(--on-surface)' }}>{c.name}</div>
                  <Icon name={inIt ? 'check_circle' : 'add_circle'} size={22} color={inIt ? 'var(--primary)' : 'var(--on-surface-variant)'} fill={inIt} weight={500} />
                </button>
              );
            })}
          </div>
          <div style={{ display: 'flex', gap: 8, padding: 12, borderTop: '1px solid var(--outline-variant)' }}>
            <input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="New collection…"
              style={{ flex: 1, height: 42, border: '1px solid var(--outline-variant)', background: 'var(--surface-container-high)', borderRadius: 12, padding: '0 14px', font: '400 14px/1 var(--font-sans)', color: 'var(--on-surface)', outline: 'none' }}
            />
            <button
              onClick={() => {
                if (name.trim()) create.mutate({ name: name.trim() }, { onSuccess: (c) => { addItem.mutate({ c, item }); setName(''); } });
              }}
              style={{ width: 44, borderRadius: 12, border: 'none', cursor: 'pointer', background: 'var(--primary)', color: 'var(--on-primary)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}
            >
              <Icon name="add" size={22} color="var(--on-primary)" weight={500} />
            </button>
          </div>
        </div>
      </Glass>
    </div>
  );
}
