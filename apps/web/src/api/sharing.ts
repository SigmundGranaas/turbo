import { apiFetch } from './client';

/** Create a share link granting `role` (default viewer) on a resource. Returns
 *  the opaque link token; the web share URL is `${origin}/?share=<token>`. */
export async function createShareLink(resourceId: string, role = 'viewer'): Promise<string> {
  const r = await apiFetch<{ linkToken: string }>('/api/sharing/grants/links', {
    method: 'POST',
    body: JSON.stringify({ resourceId, role }),
  });
  return r.linkToken;
}

export interface Redemption {
  resourceId: string;
  resourceType: string;
}

/** Redeem a link token for the signed-in user (materialises a grant; the
 *  resource then flows in via normal sync). Requires auth. */
export async function redeemLink(token: string): Promise<Redemption> {
  return apiFetch<Redemption>(`/api/sharing/grants/links/${token}/redeem`, { method: 'POST' });
}

/** The current user's profile, incl. the shareable friend code ("turbo-XXXX"). */
export async function getProfile(): Promise<{ friendCode: string }> {
  return apiFetch<{ friendCode: string }>('/api/sharing/me/profile');
}

/** Build the copyable share URL for a freshly-minted token. */
export function shareUrl(token: string): string {
  return `${window.location.origin}/?share=${encodeURIComponent(token)}`;
}

// ── Friends & groups ──────────────────────────────────────────────────────
// Same /api/sharing surface the Android client implements (SharingDtos.kt);
// the backend has served these since the Flutter era.

export interface Friendship {
  otherUserId: string;
  initiatorId: string;
  status: 'pending' | 'accepted' | 'blocked';
  createdAt: string;
  acceptedAt?: string;
}

export interface GroupMember {
  userId: string;
  role: 'admin' | 'member';
  joinedAt: string;
}

export interface FriendGroup {
  id: string;
  ownerId: string;
  name: string;
  createdAt: string;
  updatedAt: string;
  members: GroupMember[];
}

/** Friendships, optionally filtered by wire status (pending/accepted/blocked). */
export async function listFriendships(status?: Friendship['status']): Promise<Friendship[]> {
  const q = status ? `?status=${status}` : '';
  return apiFetch<Friendship[]>(`/api/sharing/friendships${q}`);
}

export async function requestFriendship(otherUserId: string): Promise<Friendship> {
  return apiFetch<Friendship>('/api/sharing/friendships/request', {
    method: 'POST',
    body: JSON.stringify({ otherUserId }),
  });
}

export async function acceptFriendship(otherUserId: string): Promise<Friendship> {
  return apiFetch<Friendship>('/api/sharing/friendships/accept', {
    method: 'POST',
    body: JSON.stringify({ otherUserId }),
  });
}

export async function removeFriendship(otherUserId: string): Promise<void> {
  await apiFetch(`/api/sharing/friendships/${otherUserId}`, { method: 'DELETE' });
}

/** Resolve a friend code ("turbo-XXXX" or the bare code) to a user id; null when unknown. */
export async function lookupUserByCode(code: string): Promise<string | null> {
  const bare = code.trim().replace(/^turbo-/i, '');
  if (!bare) return null;
  try {
    const r = await apiFetch<{ userId: string }>(`/api/sharing/users/lookup?code=${encodeURIComponent(bare)}`);
    return r.userId ?? null;
  } catch {
    return null;
  }
}

export async function listGroups(): Promise<FriendGroup[]> {
  return apiFetch<FriendGroup[]>('/api/sharing/groups');
}

export async function createGroup(name: string): Promise<FriendGroup> {
  return apiFetch<FriendGroup>('/api/sharing/groups', { method: 'POST', body: JSON.stringify({ name }) });
}

export async function deleteGroup(id: string): Promise<void> {
  await apiFetch(`/api/sharing/groups/${id}`, { method: 'DELETE' });
}

export async function addGroupMember(groupId: string, userId: string): Promise<void> {
  await apiFetch(`/api/sharing/groups/${groupId}/members`, { method: 'POST', body: JSON.stringify({ userId }) });
}

export async function removeGroupMember(groupId: string, userId: string): Promise<void> {
  await apiFetch(`/api/sharing/groups/${groupId}/members/${userId}`, { method: 'DELETE' });
}

// ── Visibility ────────────────────────────────────────────────────────────

export type ResourceVisibility = 'private' | 'friends' | 'unlisted_link' | 'public';

/** Owner-only: set how widely a resource is visible by default (grants still
 *  control per-user/group access on top). */
export async function setResourceVisibility(resourceId: string, visibility: ResourceVisibility): Promise<void> {
  await apiFetch(`/api/sharing/resources/${resourceId}/visibility`, {
    method: 'PUT',
    body: JSON.stringify({ visibility }),
  });
}
