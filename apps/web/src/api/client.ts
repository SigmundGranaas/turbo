import { API_BASE } from '../config';

export class ApiError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

// ── Transparent session refresh ──────────────────────────────────────────
// The web client is cookie-authed. The access-token cookie is short-lived
// (~15 min); the refresh-token cookie lasts 7 days. The backend exposes
// `POST /api/auth/token/refresh`, which trades the refresh cookie for a fresh
// access cookie. Without calling it, the session silently dies after 15 min —
// `session/me` and every data write start returning 401 ("session doesn't
// persist", "sign in to save"). So on a 401 we refresh once and retry the
// request. The native apps do the bearer equivalent; this is the web parity.
const REFRESH_PATH = '/api/auth/token/refresh';

// Single-flight: many requests can 401 at once (session + markers + tracks on
// load). They all await the SAME refresh so we hit the endpoint once.
let refreshInFlight: Promise<boolean> | null = null;
// After a failed refresh (anonymous visitor / revoked token) don't re-attempt
// for a beat — otherwise every public call on the page storms the endpoint.
let refreshFailedUntil = 0;

async function refreshSession(): Promise<boolean> {
  if (performance.now() < refreshFailedUntil) return false;
  if (!refreshInFlight) {
    refreshInFlight = (async () => {
      try {
        const res = await fetch(`${API_BASE}${REFRESH_PATH}`, {
          method: 'POST',
          credentials: 'include',
        });
        if (!res.ok) {
          refreshFailedUntil = performance.now() + 10_000;
          return false;
        }
        return true;
      } catch {
        refreshFailedUntil = performance.now() + 10_000;
        return false;
      } finally {
        refreshInFlight = null;
      }
    })();
  }
  return refreshInFlight;
}

/** Typed fetch against the backend. `credentials: 'include'` so the HttpOnly
 *  auth cookies set by the OAuth/login flow ride along on every request — the
 *  web client is cookie-authed (the native apps use bearer JWTs instead).
 *  On a 401 the short-lived access cookie has likely expired: we transparently
 *  refresh it from the long-lived refresh cookie and retry once before giving
 *  up. Throws [`ApiError`] on a non-2xx response. */
export async function apiFetch<T = unknown>(path: string, init?: RequestInit): Promise<T> {
  const { headers: initHeaders, ...rest } = init ?? {};
  const doFetch = () =>
    fetch(`${API_BASE}${path}`, {
      credentials: 'include',
      ...rest,
      // headers built last so caller headers (e.g. If-Match) merge on top of the
      // defaults without `...init` clobbering the whole headers object.
      headers: {
        Accept: 'application/json',
        ...(rest.body ? { 'Content-Type': 'application/json' } : {}),
        ...initHeaders,
      },
    });

  let res = await doFetch();
  // 401 → access cookie expired. Refresh once (single-flight) and retry. Skip
  // for the refresh endpoint itself to avoid recursion.
  if (res.status === 401 && path !== REFRESH_PATH) {
    if (await refreshSession()) {
      res = await doFetch();
    }
  }
  if (!res.ok) {
    throw new ApiError(res.status, `${init?.method ?? 'GET'} ${path} → ${res.status}`);
  }
  if (res.status === 204) return undefined as T;
  const ct = res.headers.get('content-type') ?? '';
  return (ct.includes('application/json') ? await res.json() : await res.text()) as T;
}
