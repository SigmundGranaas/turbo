/**
 * Auth flow for the admin SPA.
 *
 * The Turbo .NET auth service issues an `access_token` cookie after
 * a successful Google OAuth round-trip. The SPA itself never sees
 * the token — fetch's `credentials: 'include'` carries the cookie
 * on every API call, and the Rust tileserver's auth extractor
 * accepts it directly.
 *
 * Flow on cold load:
 *   1. SPA calls a probe endpoint (`GET /admin/api/resources`).
 *   2. 401 → we need to sign in. Render the SignIn screen.
 *   3. User clicks "Sign in with Google".
 *   4. SPA fetches `GET /api/auth/oauth/google/url?state=<return_path>`
 *      from the .NET auth API. State encodes where to come back to.
 *   5. `window.location.assign(authorization_url)` → Google.
 *   6. Google redirects to `/api/auth/oauth/google/callback?code=...&state=...`.
 *   7. .NET sets the access_token cookie, redirects to
 *      `${FrontendUrl}<return_path>` (the SPA URL we encoded in state).
 *   8. SPA reloads, the probe now returns 200, app renders.
 *
 * 403 with a valid token (curator role missing) renders a "not
 * authorised" screen — a different problem from "not signed in".
 */

import { useEffect, useState } from "react";
import { Outlet } from "react-router-dom";
import { api, ApiError } from "../api/client";

type AuthState =
  | { kind: "loading" }
  | { kind: "signed-in" }
  | { kind: "needs-login" }
  | { kind: "forbidden"; message: string }
  | { kind: "error"; message: string };

export function AuthGate({ children }: { children?: React.ReactNode }) {
  const [auth, setAuth] = useState<AuthState>({ kind: "loading" });

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        await api.get<unknown>("/resources");
        if (!cancelled) setAuth({ kind: "signed-in" });
      } catch (e) {
        if (cancelled) return;
        if (e instanceof ApiError) {
          if (e.status === 401) {
            // Dev-mode escape hatch: when the tileserver was started
            // with TURBO_DEV_AUTH=1, /admin/dev-login mints a curator
            // JWT cookie and returns 302. We try that first; if the
            // endpoint exists, we'll get the cookie and can retry the
            // probe. If it 404s (production), fall through to the
            // Google OAuth screen.
            try {
              const resp = await fetch("/admin/dev-login", {
                credentials: "include",
                redirect: "manual",
              });
              // `manual` redirect → opaqueredirect when the server
              // returned 3xx. Either way: try the probe again.
              if (resp.type === "opaqueredirect" || resp.ok) {
                await api.get<unknown>("/resources");
                if (!cancelled) setAuth({ kind: "signed-in" });
                return;
              }
            } catch {
              // ignore; fall through to SignInScreen
            }
            setAuth({ kind: "needs-login" });
            return;
          }
          if (e.status === 403) {
            setAuth({
              kind: "forbidden",
              message:
                "Signed in, but missing the `curator` role. Ask an admin to grant it.",
            });
            return;
          }
        }
        setAuth({ kind: "error", message: (e as Error).message });
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  if (auth.kind === "loading") {
    return <CenteredMessage>Checking sign-in…</CenteredMessage>;
  }

  if (auth.kind === "needs-login") {
    return <SignInScreen />;
  }

  if (auth.kind === "forbidden") {
    return <CenteredMessage tone="warning">{auth.message}</CenteredMessage>;
  }

  if (auth.kind === "error") {
    return (
      <CenteredMessage tone="error">
        Couldn&apos;t reach the admin API: {auth.message}
      </CenteredMessage>
    );
  }

  return <>{children ?? <Outlet />}</>;
}

function SignInScreen() {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const onSignIn = async () => {
    setBusy(true);
    setError(null);
    try {
      // Where the curator should land after the OAuth round-trip.
      // location.pathname is already under the SPA basename
      // (e.g. /admin/app/jobs). The .NET callback honours this as
      // long as it's same-origin and starts with `/`.
      const returnPath = location.pathname + location.search;
      const state = encodeURIComponent(returnPath);
      const resp = await fetch(
        `/api/auth/oauth/google/url?state=${state}`,
        { credentials: "include" },
      );
      if (!resp.ok) {
        throw new Error(`auth URL fetch failed (${resp.status})`);
      }
      const body = (await resp.json()) as { authorizationUrl: string };
      // Some servers return PascalCase, some camelCase.
      const url =
        body.authorizationUrl ??
        (body as unknown as { AuthorizationUrl: string }).AuthorizationUrl;
      if (!url) throw new Error("auth URL missing from response");
      window.location.assign(url);
    } catch (e) {
      setError((e as Error).message);
      setBusy(false);
    }
  };

  return (
    <div className="min-h-screen flex items-center justify-center bg-ink-50">
      <div className="max-w-sm w-full p-6 rounded border border-ink-200 bg-white shadow-sm">
        <div className="text-lg font-semibold">Turbo Admin</div>
        <p className="text-sm text-ink-500 mt-2 mb-6">
          Sign in with Google to manage curated paths and ingest jobs.
        </p>
        <button
          type="button"
          onClick={onSignIn}
          disabled={busy}
          className="w-full px-4 py-2 rounded bg-ink-900 text-ink-50 hover:bg-ink-700 disabled:opacity-50 text-sm"
          data-testid="signin-button"
        >
          {busy ? "Redirecting to Google…" : "Sign in with Google"}
        </button>
        {error && (
          <div className="mt-4 text-sm text-red-700" data-testid="signin-error">
            {error}
          </div>
        )}
      </div>
    </div>
  );
}

function CenteredMessage({
  children,
  tone = "info",
}: {
  children: React.ReactNode;
  tone?: "info" | "warning" | "error";
}) {
  const toneClass =
    tone === "warning"
      ? "border-amber-300 bg-amber-50 text-amber-900"
      : tone === "error"
        ? "border-red-300 bg-red-50 text-red-900"
        : "border-ink-200 bg-white text-ink-700";
  return (
    <div className="min-h-screen flex items-center justify-center bg-ink-50">
      <div
        className={`max-w-md w-full p-6 rounded border ${toneClass} shadow-sm text-sm`}
      >
        {children}
      </div>
    </div>
  );
}
