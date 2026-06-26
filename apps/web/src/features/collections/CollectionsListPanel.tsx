import { useState } from 'react';
import type { Collection } from '../../api/collections';
import { usePaths } from '../../store/pathsStore';
import { useCollectionMutations } from './useCollections';
import { SidePanel, Tabs } from '../../ui/Panel';
import { Icon } from '../../ui/Icon';

/** Collections grid (a tab of the Saved panel). */
export function CollectionsListPanel({
  dark,
  collections,
  loading,
  onOpen,
  onClose,
}: {
  dark: boolean;
  collections: Collection[];
  loading: boolean;
  onOpen: (id: string) => void;
  onClose: () => void;
}) {
  const { create } = useCollectionMutations();
  const [name, setName] = useState('');

  return (
    <SidePanel dark={dark} title="Saved" subtitle={`${collections.length} collection${collections.length === 1 ? '' : 's'}`} onClose={onClose}>
      <div style={{ paddingTop: 8, paddingBottom: 12 }}>
        <Tabs items={[{ label: 'Paths' }, { label: 'Collections' }]} active={1} onPick={(i) => i === 0 && usePaths.getState().setTab('paths')} />

        <div style={{ display: 'flex', gap: 8, margin: '16px 0' }}>
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="New collection…"
            style={{ flex: 1, height: 40, border: '1px solid var(--outline-variant)', background: 'var(--surface-container-high)', borderRadius: 12, padding: '0 14px', font: '400 14px/1 var(--font-sans)', color: 'var(--on-surface)', outline: 'none' }}
          />
          <button
            onClick={() => {
              if (name.trim()) create.mutate({ name: name.trim() }, { onSuccess: () => setName('') });
            }}
            style={{ width: 44, borderRadius: 12, border: 'none', cursor: 'pointer', background: 'var(--primary)', color: 'var(--on-primary)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}
          >
            <Icon name="add" size={22} color="var(--on-primary)" weight={500} />
          </button>
        </div>

        {loading && <div style={{ font: '400 14px/20px var(--font-sans)', color: 'var(--on-surface-variant)' }}>Loading…</div>}
        {!loading && collections.length === 0 && (
          <div style={{ font: '400 14px/21px var(--font-sans)', color: 'var(--on-surface-variant)' }}>No collections yet — create one above.</div>
        )}
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
          {collections.map((c) => (
            <button
              key={c.id}
              onClick={() => onOpen(c.id)}
              style={{ textAlign: 'left', border: '1px solid var(--outline-variant)', background: 'var(--surface-container-low)', borderRadius: 18, padding: 0, overflow: 'hidden', cursor: 'pointer' }}
            >
              <div style={{ height: 56, background: c.colorHex ?? 'var(--primary)' }} />
              <div style={{ padding: '10px 12px' }}>
                <div style={{ font: '600 14px/18px var(--font-sans)', color: 'var(--on-surface)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{c.name}</div>
                <div style={{ font: '400 12px/15px var(--font-sans)', color: 'var(--on-surface-variant)', marginTop: 1 }}>{c.items.length} item{c.items.length === 1 ? '' : 's'}</div>
              </div>
            </button>
          ))}
        </div>
      </div>
    </SidePanel>
  );
}
