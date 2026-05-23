/**
 * Thin fetch wrapper. All admin endpoints live at /admin/api/* — the
 * Rust tileserver mounts them at /admin/* and Vite's dev proxy + the
 * YARP gateway both forward there. We rely on the access_token cookie
 * set by the .NET auth service rather than threading a bearer token
 * through the SPA; the cookie domain shares /api/ with the auth
 * endpoints.
 */

export class ApiError extends Error {
  readonly status: number;
  readonly body: unknown;
  constructor(status: number, body: unknown, message?: string) {
    super(message ?? `HTTP ${status}`);
    this.status = status;
    this.body = body;
  }
}

const BASE = "/admin/api";

async function request<T>(
  path: string,
  init: RequestInit = {},
): Promise<T> {
  const headers = new Headers(init.headers ?? {});
  if (!headers.has("Accept")) headers.set("Accept", "application/json");
  if (init.body && !headers.has("Content-Type") && !(init.body instanceof FormData)) {
    headers.set("Content-Type", "application/json");
  }
  const res = await fetch(`${BASE}${path}`, {
    credentials: "include",
    ...init,
    headers,
  });
  if (!res.ok) {
    let body: unknown;
    try {
      body = await res.json();
    } catch {
      body = await res.text();
    }
    throw new ApiError(res.status, body);
  }
  if (res.status === 204) return undefined as T;
  return (await res.json()) as T;
}

export const api = {
  get: <T,>(path: string) => request<T>(path),
  post: <T,>(path: string, body: unknown) =>
    request<T>(path, { method: "POST", body: JSON.stringify(body) }),
  put: <T,>(path: string, body: unknown) =>
    request<T>(path, { method: "PUT", body: JSON.stringify(body) }),
  del: <T,>(path: string) => request<T>(path, { method: "DELETE" }),
  postForm: <T,>(path: string, form: FormData) =>
    request<T>(path, { method: "POST", body: form }),
};
