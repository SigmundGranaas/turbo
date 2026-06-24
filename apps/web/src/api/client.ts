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

/** Typed fetch against the backend. `credentials: 'include'` so the HttpOnly
 *  auth cookies set by the OAuth/login flow ride along on every request — the
 *  web client is cookie-authed (the native apps use bearer JWTs instead).
 *  Throws [`ApiError`] on a non-2xx response. */
export async function apiFetch<T = unknown>(path: string, init?: RequestInit): Promise<T> {
  const { headers: initHeaders, ...rest } = init ?? {};
  const res = await fetch(`${API_BASE}${path}`, {
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
  if (!res.ok) {
    throw new ApiError(res.status, `${init?.method ?? 'GET'} ${path} → ${res.status}`);
  }
  if (res.status === 204) return undefined as T;
  const ct = res.headers.get('content-type') ?? '';
  return (ct.includes('application/json') ? await res.json() : await res.text()) as T;
}
