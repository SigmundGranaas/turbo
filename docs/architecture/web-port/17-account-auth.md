# 17 — Account + auth

> Let a web user register / log in with email+password or Google, stay logged in across reloads via cookies, see their account (email, friend code, sync status), and log out — while the app stays usable signed-out for read-only/local work.

## Status
- **Android (gold standard):**
  - **Email/password login + register** (`AuthViewModel`, `AuthScreen`): a single form with a Login/Register toggle (`AuthMode`), a password ≥ 6 rule (`MIN_PASSWORD = 6`), a confirm field on register, inline error display, and a loading state. Calls `POST /api/auth/auth/login` / `/register`.
  - **Google sign-in:** custom-tab consent (`beginGoogleSignIn` → `GET /api/auth/oauth/google/url` → open URL) then `completeGoogleSignIn(code)` exchanges the redirect code (`POST /api/auth/oauth/mobile-signin`).
  - **Session restore on launch:** `AuthRepository.restore()` loads persisted tokens; `AuthState` is `Unknown` → resolves to `SignedOut` or `SignedIn(account)`.
  - **Account screen** (`AccountView`): email, friend code "turbo-XXXX" (copyable), sync status (Idle/Syncing/Failed) + **Sync now**, links to Friends/Groups (doc 16), and **Sign out** (`POST /api/auth/token/revoke`).
- **Web today:** **partial.** `api/auth.ts` has `useSession()` (TanStack Query over `GET /api/auth/session/me`), `loginWithGoogle(returnTo?)` (`GET /api/auth/oauth/google/url`), and `logout()` (`POST /api/auth/token/revoke`). `apiFetch` already uses `credentials: 'include'` with `VITE_API_BASE`. **MISSING:** email/password login + register forms, the account screen, and any signed-in UI gating.
- **Renderer/back-end prerequisites:**
  - **No renderer change.**
  - Cookie-based session: the server sets HttpOnly `.sandring.no` cookies on login/register/refresh and the OAuth callback; the web never handles raw tokens.
  - Endpoints: `POST /api/auth/auth/login`, `POST /api/auth/auth/register`, `GET /api/auth/oauth/google/url?returnTo=`, OAuth `…/callback` (server-set cookies + redirect back), `GET /api/auth/session/me`, `POST /api/auth/token/refresh`, `POST /api/auth/token/revoke`. Friend code from `GET /api/sharing/me/profile` (doc 16); sync status from doc 18.

## User stories

### 1. Register with email + password
*As a new user, I want to create an account with email and password, so that I can sync and share.*

**Acceptance criteria**
- A register form collects email, password, and confirm-password; submit is disabled until email is non-blank, password ≥ 6 chars, and confirm matches.
- Submitting `POST /api/auth/auth/register` on success sets the session cookie server-side; the app refetches `useSession()` and transitions to signed-in without a full reload.
- Inline, field-level errors are shown for: password too short, mismatched confirm, invalid email, and server validation failures.
- A duplicate-email / already-registered response shows a clear "An account with this email already exists — log in instead" message and offers to switch to Login mode.

**Web-specific notes**
- The form is the same component as Login with a mode toggle (mirrors Android `AuthMode`). The confirm field and the ≥ 6 rule are client-validated before the network call.

### 2. Log in with email + password
*As a returning user, I want to log in with my email and password, so that I get my data back.*

**Acceptance criteria**
- Login form posts `POST /api/auth/auth/login`; success sets cookies and `useSession()` resolves to the signed-in user.
- **Bad credentials** (401) show "Incorrect email or password" without clearing the email field.
- A loading state disables the submit button and shows a spinner during the request.
- Toggling Login↔Register preserves the entered email and clears password fields and errors.

**Web-specific notes**
- No tokens are stored in JS/localStorage; the session lives entirely in the HttpOnly cookie. `apiFetch` already sends `credentials:'include'`.

### 3. Log in with Google
*As a user, I want to sign in with Google, so that I don't manage another password.*

**Acceptance criteria**
- "Continue with Google" calls `GET /api/auth/oauth/google/url?returnTo=<current path>` and redirects the browser to the returned consent URL (full-page navigation, not a popup, to match cookie/redirect flow).
- After consent, Google redirects to the API callback, which sets `.sandring.no` cookies and redirects back to the web app at `returnTo`.
- On return, the app refetches `useSession()`; finding a session, it lands the user signed-in on the page they started from.
- **OAuth cancel / error return** (user denies consent, or callback returns an error param): the app lands back signed-out on the auth screen with a non-blocking "Google sign-in was cancelled" notice — no crash, no stuck spinner.

**Web-specific notes**
- Unlike Android's custom-tab + `mobile-signin` code exchange, the web uses the **redirect callback** that sets cookies directly (`…/oauth/google/callback`); the web never sees the `code`. The current `loginWithGoogle` already starts this redirect.
- `returnTo` must be validated server-side / restricted to the app origin (open-redirect guard); the web should pass only an in-app path.

### 4. Stay logged in across reloads (session restore)
*As a user, I want to stay logged in when I reload or reopen the tab, so that I'm not re-authenticating constantly.*

**Acceptance criteria**
- On app boot, `useSession()` (queryKey `['session']`) calls `GET /api/auth/session/me`; a valid cookie resolves to the signed-in user, mirroring Android's `restore()`.
- While the first session check is in flight, the UI shows a neutral "checking" state (equivalent to Android `AuthState.Unknown`) — it does **not** flash the signed-out auth screen.
- **Expired access token but valid refresh:** a 401 from any API call triggers a single `POST /api/auth/token/refresh`; on success the original request is retried transparently and the session persists. On refresh failure the session resolves to signed-out and protected views revert to read-only/local.
- `useSession()` refetches on window focus so a session revoked elsewhere is detected without a manual reload.

**Web-specific notes**
- Implement the 401→refresh→retry once in `apiFetch` (single-flight refresh; don't loop). This is the web analogue of Android's `AuthorizedHttp`.

### 5. View my account
*As a signed-in user, I want an account screen showing my email, friend code, and sync status, so that I can confirm who I am and manage sync.*

**Acceptance criteria**
- The account screen shows the signed-in email (from `session/me`), the friend code "turbo-XXXX" (`GET /api/sharing/me/profile`, copyable), and the current sync status (doc 18) with a **Sync now** button.
- It links to Friends/Groups/Shared management (doc 16) and to Settings (doc 19).
- Friend code copy uses the clipboard helper (doc 16) with a manual-select fallback.
- A **Sign out** button is present (story 6).

**Web-specific notes**
- Sync status on web (online-first) is mostly "live / last refreshed" rather than a long-running queue — doc 18 defines exactly what the indicator shows.

### 6. Log out
*As a user, I want to log out, so that my account isn't accessible on a shared computer.*

**Acceptance criteria**
- "Sign out" calls `POST /api/auth/token/revoke`, which clears the `.sandring.no` cookies server-side; the app clears TanStack Query caches for account-scoped data (`session`, sharing, sync, server settings) and returns to the signed-out shell.
- After logout the app is still usable in read-only/local mode (story 7); no forced redirect to a login wall.
- A failed revoke (network error) still optimistically signs out locally and surfaces "Signed out (server cleanup pending)".

### 7. Signed-out app stays usable
*As a visitor without an account, I want to use the map and public features, so that I'm not blocked by a login wall.*

**Acceptance criteria**
- Signed-out, the app renders the map, base layers (doc 01), search, routing, and public conditions normally; only account-scoped features (sync, sharing, my markers/tracks/collections) prompt for sign-in when invoked.
- Account-scoped actions show an inline sign-in prompt (not a hard redirect) and return the user to where they were after auth.
- No 401 from a protected call ever crashes the app; it degrades to the signed-out state for that feature.

**Web-specific notes**
- This mirrors the README's "signed-out app still usable" requirement; auth gates are per-action, not a global route guard.

## Primary flows (web)

**Happy path — register**
1. User opens the auth screen, toggles to Register, enters email + password (≥6) + confirm.
2. `POST /api/auth/auth/register` → cookies set → `useSession()` refetch → signed-in shell.

**Happy path — Google**
1. User clicks "Continue with Google" → `GET /api/auth/oauth/google/url?returnTo=/map` → browser redirects to Google.
2. Consent → API callback sets cookies → redirect back to `/map`.
3. App boot `useSession()` finds session → signed-in.

**Happy path — session restore**
1. App boots, shows "checking" state, calls `GET /api/auth/session/me`.
2. Valid cookie → signed-in; no cookie → signed-out shell (read-only).

**Edge — bad credentials**
- Login 401 → "Incorrect email or password", email preserved, password cleared.

**Edge — OAuth cancel/error**
- Callback returns error / user denies → land signed-out with a dismissible notice.

**Edge — expired session**
- API 401 → single `token/refresh`; success → retry + stay signed-in; failure → signed-out, feature reverts to read-only.

**Edge — offline / network error**
- Login/register network failure → "Can't reach the server — check your connection" with retry; no token state mutated.

## UI / UX on web
- **Auth screen:** a centered card (desktop) / full-screen sheet (mobile) with the email/password form, a Login/Register segmented toggle, "Continue with Google", and inline errors. Reachable from a top-bar **Account** button and from any per-action sign-in prompt.
- **Account screen:** a side panel (desktop) / route (mobile) showing email, friend code (copyable), sync status + Sync now, links to Friends/Groups (doc 16) and Settings (doc 19), and Sign out.
- **Top bar:** an account avatar/menu — signed-out shows "Sign in"; signed-in shows initials/email and a menu to Account / Sign out.
- **Composition with canvas:** auth/account UI overlays the shell; on mobile the account route can be a full sheet (no `set_viewport_inset` needed since it covers the map).

## Data & APIs
- **Auth:** `POST /api/auth/auth/login` (`{email,password}`), `POST /api/auth/auth/register` (`{email,password,confirmPassword}`) → server sets cookies (response also carries account info). `GET /api/auth/oauth/google/url?returnTo=` → `{ authorizationUrl }`; callback sets cookies + redirects. `GET /api/auth/session/me` → `{ accountId, email, roles, isActive }` (401 signed-out). `POST /api/auth/token/refresh`, `POST /api/auth/token/revoke`. *(login/register/oauth-url public; session/me requires cookie)*
- **Friend code:** `GET /api/sharing/me/profile` → `{ userId, friendCode }` (doc 16). *(auth)*
- **State:** TanStack Query `['session']` (staleTime ~30s, refetch on focus) is the single source of auth truth; no Zustand auth slice (matches current web). Account-scoped query keys are invalidated/cleared on login + logout.
- **Client helpers (`api/auth.ts`):** extend with `login(email,password)`, `register(email,password,confirm)` (both `apiFetch` POST + invalidate `['session']`); `useSession`, `loginWithGoogle`, `logout` already exist.
- **External:** Google OAuth (via the API's redirect flow only — web never holds the code).

## Renderer integration
- **None.** Auth/account is pure app-shell + API. The map canvas is unaffected by sign-in state except that account-scoped overlays (my markers/tracks) populate once signed-in (docs 06/08/18).

## Out of scope (this phase)
- Password reset / forgot-password and email verification flows (confirm backend support before scoping).
- Account deletion / data export from the account screen.
- Additional OAuth providers beyond Google.
- Multi-account switching on one browser.
- Storing tokens in JS (the web is cookie-only by design).

## Open questions
- **Refresh placement:** confirm `apiFetch` should own the single-flight 401→refresh→retry, and that `/token/refresh` works purely from the cookie (no body) for the web.
- **`returnTo` allowlist:** confirm the server restricts the OAuth `returnTo`/`state` to the app origin (open-redirect guard) and which param name it expects (`returnTo` vs `state`).
- **Account-info on login response:** does the login/register JSON body include email/accountId for an immediate optimistic signed-in state, or must the web always follow with `session/me`?
- **Password reset:** is there a backend endpoint? If not, note as a known gap on the auth screen.
