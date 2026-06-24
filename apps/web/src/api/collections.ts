import { ApiError, apiFetch } from './client';

export type CollectionItemType = 'marker' | 'path';

export interface CollectionItem {
  type: CollectionItemType;
  uuid: string;
}

/** A user collection grouping markers + paths (`/api/collections/Collections`).
 *  See port doc 15. */
export interface Collection {
  id: string;
  name: string;
  colorHex?: string;
  iconKey?: string;
  items: CollectionItem[];
  version: number;
}

interface CollectionResponse {
  id: string;
  name: string;
  colorHex?: string;
  iconKey?: string;
  items?: { type: string; uuid: string }[];
  version?: number;
}

const fromApi = (r: CollectionResponse): Collection => ({
  id: r.id,
  name: r.name,
  colorHex: r.colorHex,
  iconKey: r.iconKey,
  items: (r.items ?? []).map((i) => ({ type: i.type as CollectionItemType, uuid: i.uuid })),
  version: r.version ?? 1,
});

export async function listCollections(): Promise<Collection[]> {
  try {
    const r = await apiFetch<{ items: CollectionResponse[] }>('/api/collections/Collections');
    return (r.items ?? []).map(fromApi);
  } catch (e) {
    if (e instanceof ApiError && (e.status === 401 || e.status === 403)) return [];
    throw e;
  }
}

export async function createCollection(name: string, colorHex?: string): Promise<Collection> {
  const r = await apiFetch<CollectionResponse>('/api/collections/Collections', {
    method: 'POST',
    body: JSON.stringify({ name, colorHex }),
  });
  return fromApi(r);
}

export async function updateCollection(c: Collection, changes: { name?: string; colorHex?: string }): Promise<Collection> {
  const r = await apiFetch<CollectionResponse>(`/api/collections/Collections/${c.id}`, {
    method: 'PUT',
    headers: { 'If-Match': String(c.version) },
    body: JSON.stringify(changes),
  });
  return fromApi(r);
}

export async function deleteCollection(c: Collection): Promise<void> {
  await apiFetch(`/api/collections/Collections/${c.id}`, { method: 'DELETE', headers: { 'If-Match': String(c.version) } });
}

export async function addItem(c: Collection, item: CollectionItem): Promise<void> {
  await apiFetch(`/api/collections/Collections/${c.id}/items`, {
    method: 'POST',
    headers: { 'If-Match': String(c.version) },
    body: JSON.stringify({ type: item.type, uuid: item.uuid }),
  });
}

export async function removeItem(c: Collection, item: CollectionItem): Promise<void> {
  await apiFetch(`/api/collections/Collections/${c.id}/items/${item.type}/${item.uuid}`, {
    method: 'DELETE',
    headers: { 'If-Match': String(c.version) },
  });
}
