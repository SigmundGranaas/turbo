import { useQuery } from '@tanstack/react-query';
import { API_BASE } from '../config';
import { ApiError, apiFetch } from './client';

export interface SessionUser {
  id: string;
  email?: string;
  name?: string;
}

/** Validate the current cookie session. `null` when unauthenticated (the
 *  backend returns 401, which we map to "logged out" rather than an error). */
export async function getSession(): Promise<SessionUser | null> {
  try {
    return await apiFetch<SessionUser>('/api/auth/session/me');
  } catch (e) {
    if (e instanceof ApiError && (e.status === 401 || e.status === 403)) return null;
    throw e;
  }
}

/** TanStack Query hook for the session. Short stale time; refetched on focus so
 *  a login in another tab reflects here. */
export function useSession() {
  return useQuery({
    queryKey: ['session'],
    queryFn: getSession,
    staleTime: 30_000,
  });
}

/** Kick off the Google OAuth redirect flow: ask the backend for the provider
 *  URL (carrying a return path in `state`), then navigate there. The callback
 *  sets `.sandring.no` cookies and redirects back. */
export async function loginWithGoogle(returnTo: string = window.location.href): Promise<void> {
  const { url } = await apiFetch<{ url: string }>(
    `/api/auth/oauth/google/url?returnTo=${encodeURIComponent(returnTo)}`,
  );
  window.location.assign(url);
}

/** Email/password sign-in. On success the server sets the `.sandring.no` auth
 *  cookies; refetch the session afterwards. Throws [`ApiError`] on bad creds. */
export async function loginWithPassword(email: string, password: string): Promise<void> {
  await apiFetch('/api/auth/Auth/login', { method: 'POST', body: JSON.stringify({ email, password }) });
}

/** Register a new email/password account (also sets cookies on success). */
export async function registerWithPassword(email: string, password: string, confirmPassword: string): Promise<void> {
  await apiFetch('/api/auth/Auth/register', { method: 'POST', body: JSON.stringify({ email, password, confirmPassword }) });
}

/** Revoke the refresh token + clear auth cookies, then reload to a clean state. */
export async function logout(): Promise<void> {
  await fetch(`${API_BASE}/api/auth/token/revoke`, { method: 'POST', credentials: 'include' });
}
