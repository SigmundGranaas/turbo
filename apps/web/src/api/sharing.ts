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
