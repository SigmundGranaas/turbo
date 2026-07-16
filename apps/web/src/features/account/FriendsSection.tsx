import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import {
  acceptFriendship,
  addGroupMember,
  createGroup,
  deleteGroup,
  listFriendships,
  listGroups,
  lookupUserByCode,
  removeFriendship,
  requestFriendship,
  type Friendship,
} from '../../api/sharing';
import { Eyebrow } from '../../ui/Panel';
import { Btn } from '../../ui/Glass';
import { Icon } from '../../ui/Icon';

/** Friends & groups management (signed-in only): add a friend by their
 *  "turbo-XXXX" code, accept/decline incoming requests, and maintain share
 *  groups. The backend has carried these endpoints since the Flutter app;
 *  this restores the client surface on web. Users are identified by friend
 *  code — the API exposes no display names, so rows show a shortened id. */
export function FriendsSection() {
  const qc = useQueryClient();
  const friendships = useQuery({ queryKey: ['friendships'], queryFn: () => listFriendships() });
  const groups = useQuery({ queryKey: ['groups'], queryFn: listGroups });

  const [code, setCode] = useState('');
  const [msg, setMsg] = useState<string | null>(null);
  const [groupName, setGroupName] = useState('');
  const [memberCodeFor, setMemberCodeFor] = useState<string | null>(null);
  const [memberCode, setMemberCode] = useState('');

  const refresh = () => {
    void qc.invalidateQueries({ queryKey: ['friendships'] });
    void qc.invalidateQueries({ queryKey: ['groups'] });
  };

  const addFriend = async () => {
    setMsg(null);
    const userId = await lookupUserByCode(code);
    if (!userId) {
      setMsg('No user with that friend code.');
      return;
    }
    try {
      await requestFriendship(userId);
      setCode('');
      setMsg('Request sent.');
      refresh();
    } catch {
      setMsg('Couldn’t send the request.');
    }
  };

  const addMember = async (groupId: string) => {
    setMsg(null);
    const userId = await lookupUserByCode(memberCode);
    if (!userId) {
      setMsg('No user with that friend code.');
      return;
    }
    try {
      await addGroupMember(groupId, userId);
      setMemberCode('');
      setMemberCodeFor(null);
      refresh();
    } catch {
      setMsg('Couldn’t add the member.');
    }
  };

  const all = friendships.data ?? [];
  // An incoming request is one the OTHER user initiated.
  const incoming = all.filter((f) => f.status === 'pending' && f.initiatorId === f.otherUserId);
  const outgoing = all.filter((f) => f.status === 'pending' && f.initiatorId !== f.otherUserId);
  const accepted = all.filter((f) => f.status === 'accepted');

  const userLabel = (f: Friendship) => `user ${f.otherUserId.slice(0, 8)}…`;
  const rowStyle = { display: 'flex', alignItems: 'center', gap: 8, padding: '6px 2px', font: '400 13px/18px var(--font-sans)', color: 'var(--on-surface)' } as const;

  return (
    <>
      <Eyebrow style={{ margin: '24px 0 10px' }}>Friends</Eyebrow>
      <div style={{ display: 'flex', gap: 8 }}>
        <input
          value={code}
          onChange={(e) => setCode(e.target.value)}
          placeholder="turbo-XXXX"
          style={{ flex: 1, padding: '8px 10px', borderRadius: 10, border: '1px solid var(--outline)', background: 'var(--surface)', color: 'var(--on-surface)', font: '400 13px/16px var(--font-sans)' }}
        />
        <Btn label="Add" size="sm" onClick={() => void addFriend()} />
      </div>
      {msg && <div style={{ font: '400 12px/16px var(--font-sans)', color: 'var(--on-surface-variant)', marginTop: 6 }}>{msg}</div>}

      {incoming.length > 0 && (
        <>
          <div style={{ font: '600 12px/16px var(--font-sans)', color: 'var(--on-surface-variant)', margin: '10px 0 2px' }}>Requests</div>
          {incoming.map((f) => (
            <div key={f.otherUserId} style={rowStyle}>
              <Icon name="person_add" size={16} color="var(--primary)" />
              <span style={{ flex: 1 }}>{userLabel(f)}</span>
              <Btn label="Accept" size="sm" onClick={() => void acceptFriendship(f.otherUserId).then(refresh)} />
              <Btn label="Decline" size="sm" tone="surface" onClick={() => void removeFriendship(f.otherUserId).then(refresh)} />
            </div>
          ))}
        </>
      )}
      {accepted.map((f) => (
        <div key={f.otherUserId} style={rowStyle}>
          <Icon name="person" size={16} color="var(--primary)" />
          <span style={{ flex: 1 }}>{userLabel(f)}</span>
          <button
            title="Remove friend"
            onClick={() => void removeFriendship(f.otherUserId).then(refresh)}
            style={{ border: 'none', background: 'transparent', cursor: 'pointer', color: 'var(--on-surface-variant)', padding: 2 }}
          >
            <Icon name="close" size={15} />
          </button>
        </div>
      ))}
      {outgoing.map((f) => (
        <div key={f.otherUserId} style={{ ...rowStyle, color: 'var(--on-surface-variant)' }}>
          <Icon name="schedule" size={16} />
          <span style={{ flex: 1 }}>{userLabel(f)} · pending</span>
        </div>
      ))}
      {friendships.isSuccess && all.length === 0 && (
        <div style={{ font: '400 12px/17px var(--font-sans)', color: 'var(--on-surface-variant)', marginTop: 8 }}>
          No friends yet — swap friend codes to share tracks and markers directly.
        </div>
      )}

      <Eyebrow style={{ margin: '20px 0 10px' }}>Groups</Eyebrow>
      {(groups.data ?? []).map((g) => (
        <div key={g.id}>
          <div style={rowStyle}>
            <Icon name="group" size={16} color="var(--primary)" />
            <span style={{ flex: 1 }}>{g.name}</span>
            <span style={{ color: 'var(--on-surface-variant)' }}>{g.members.length}</span>
            <button
              title="Add member"
              onClick={() => setMemberCodeFor(memberCodeFor === g.id ? null : g.id)}
              style={{ border: 'none', background: 'transparent', cursor: 'pointer', color: 'var(--primary)', padding: 2 }}
            >
              <Icon name="person_add" size={16} />
            </button>
            <button
              title="Delete group"
              onClick={() => void deleteGroup(g.id).then(refresh)}
              style={{ border: 'none', background: 'transparent', cursor: 'pointer', color: 'var(--on-surface-variant)', padding: 2 }}
            >
              <Icon name="close" size={15} />
            </button>
          </div>
          {memberCodeFor === g.id && (
            <div style={{ display: 'flex', gap: 8, margin: '2px 0 8px 24px' }}>
              <input
                value={memberCode}
                onChange={(e) => setMemberCode(e.target.value)}
                placeholder="turbo-XXXX"
                style={{ flex: 1, padding: '7px 9px', borderRadius: 10, border: '1px solid var(--outline)', background: 'var(--surface)', color: 'var(--on-surface)', font: '400 13px/16px var(--font-sans)' }}
              />
              <Btn label="Add" size="sm" onClick={() => void addMember(g.id)} />
            </div>
          )}
        </div>
      ))}
      <div style={{ display: 'flex', gap: 8, marginTop: 6 }}>
        <input
          value={groupName}
          onChange={(e) => setGroupName(e.target.value)}
          placeholder="New group name"
          style={{ flex: 1, padding: '8px 10px', borderRadius: 10, border: '1px solid var(--outline)', background: 'var(--surface)', color: 'var(--on-surface)', font: '400 13px/16px var(--font-sans)' }}
        />
        <Btn
          label="Create"
          size="sm"
          tone="tonal"
          onClick={() => {
            if (!groupName.trim()) return;
            void createGroup(groupName.trim()).then(() => { setGroupName(''); refresh(); });
          }}
        />
      </div>
    </>
  );
}
