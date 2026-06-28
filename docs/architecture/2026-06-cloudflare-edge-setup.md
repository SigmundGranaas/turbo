# Cloudflare Edge + DO Droplet — Performance Completion Guide

**Status:** action checklist (manual steps on Cloudflare + the DO droplet).
**Owner action required** — these are outside the cluster/repo; ArgoCD can't do them.

---

## 1. Why we're doing this

Measured 2026-06-27 (`/api/places/search`):

| Layer | Latency (warm) |
|---|---|
| Postgres query (indexed) | 1–3 ms |
| `turbo-places` service (in-cluster) | **7 ms** p50 |
| Cluster edge (LAN → Traefik → service) | 11 ms |
| **Public via DO droplet** | **~139 ms warm / ~270 ms cold** |

The service is already fast. The ~130 ms is pure transport:

```
TODAY:    client ──45ms──► DO droplet (nginx, 134.209.202.236)
                            └─tunnel─► home cluster (192.168.1.210, behind NAT)
                                        └─► Traefik ─► turbo-places / tileserver
          (TLS handshake re-done per non-pooled client; tiles have NO edge cache)

TARGET:   client ──~5ms──► Cloudflare PoP ──(warm pooled, CF backbone)──► DO droplet ─tunnel─► home cluster
                            └─ /tiles/* and /api/places/search|reverse served from edge cache (never touch home)
```

`sandring.no` is **already a Cloudflare zone** (NS `adele/eugene.ns.cloudflare.com`), but
`kart-api` and `kart` are **grey-clouded (DNS-only)** — they point straight at the droplet
and bypass the edge. The whole fix is: make the droplet a clean Cloudflare origin, then turn
the orange cloud on and add cache rules.

**Expected after:** cached responses ~5 ms, uncached ~40–60 ms (one CF↔origin hop instead of
the client's 45 ms + re-handshake), and expensive tile renders happen **once globally** instead
of per client.

Prereqs already done in the repo:
- Tiles are namespaced under **`/tiles/*`** on `kart-api` (`tileserver-tiles-strip` middleware
  + `tileserver-tiles-ingress`; `PUBLIC_BASE_URL=https://kart-api.sandring.no/tiles`). So a
  single Cache Rule on `/tiles/*` covers every tile asset.
- Tiles already send `Cache-Control: public, max-age=86400, stale-while-revalidate=604800`
  (fonts: `immutable`); style.json `max-age=3600`. Cloudflare will honour these once the path
  is marked cache-eligible.

---

## 2. Order of operations (do not reorder)

1. **Droplet first** (§3): valid origin cert, nginx upstream keepalive, real-client-IP, don't
   strip cache headers. — Safe, no user-visible change.
2. **Cloudflare SSL mode** → Full (strict) (§4.1). — Safe.
3. **Flip the orange cloud** on `kart-api` (and `kart`) (§4.2). — The cutover. Instantly
   reversible (toggle back to grey).
4. **Add Cache Rules** (§4.3). — Additive.
5. **Verify** (§5).
6. **Only after verified:** lock the droplet firewall to Cloudflare IPs (§3.5). — Do this last
   so a mistake can't lock you out of a working path.

Rollback at any point: set the DNS record back to **DNS-only (grey)** — traffic goes straight
to the droplet exactly as today.

---

## 3. DO droplet (nginx origin) — `134.209.202.236`

> You SSH in as you normally do. These edits are in the nginx config for the
> `kart-api.sandring.no` (and `kart.sandring.no`) server blocks. Run `nginx -t` before every
> reload; `sudo systemctl reload nginx` to apply.

### 3.1 Valid origin TLS cert (required for Cloudflare "Full (strict)")

Cloudflare in Full-strict connects to the droplet over HTTPS and **validates the cert**. Two options:

- **Keep your existing Let's Encrypt cert** if the droplet already serves a valid cert for
  `kart-api.sandring.no` (it does today — we reach it over HTTPS). Nothing to do. Make sure
  certbot auto-renew still works once the orange cloud is on (use the DNS-01 challenge, or keep
  an HTTP-01 allow for `/.well-known/acme-challenge/` — see note in §3.5).
- **Or use a Cloudflare Origin CA cert** (never expires for 15 y, only trusted by Cloudflare):
  CF dashboard → SSL/TLS → **Origin Server** → **Create Certificate** → install the cert+key on
  the droplet and point `ssl_certificate` / `ssl_certificate_key` at them. Simplest long-term
  because there's no renewal and no ACME challenge to keep open.

### 3.2 Upstream keepalive (the biggest droplet-side win)

Right now nginx very likely opens a **fresh TCP connection to the home tunnel per request** —
that's an extra tunnel round-trip (~tens of ms) on every call. Pool them:

```nginx
# Pool of warm connections to the home cluster (your tunnel endpoint to Traefik).
# Replace the address with however nginx currently reaches home (WireGuard/Tailscale
# IP, or the existing proxy_pass target).
upstream home_cluster {
    server 10.0.0.2:80;          # <-- your current home-Traefik target
    keepalive 64;                # warm connections kept open
    keepalive_timeout 60s;
    keepalive_requests 10000;
}

server {
    listen 443 ssl;
    http2 on;
    server_name kart-api.sandring.no;

    # ... ssl_certificate / ssl_certificate_key ...

    location / {
        proxy_pass http://home_cluster;

        # REQUIRED for keepalive to the upstream:
        proxy_http_version 1.1;
        proxy_set_header Connection "";      # remove the default "close"

        proxy_set_header Host              $host;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header X-Forwarded-For   $proxy_add_x_forwarded_for;

        # Don't let nginx swallow the brotli/gzip the origin already does, and
        # don't rewrite Cache-Control (Cloudflare needs the origin's headers):
        proxy_pass_header  Cache-Control;
        proxy_buffering    on;
    }
}
```

Apply the same `proxy_http_version 1.1; proxy_set_header Connection "";` to the
`kart.sandring.no` server block (the web SPA) too.

### 3.3 Restore the real client IP

Once Cloudflare proxies, every request arrives from a Cloudflare IP. Restore the true client IP
so logs/rate-limits/`X-Forwarded-For` stay meaningful:

```nginx
# /etc/nginx/conf.d/cloudflare-realip.conf
# Source of truth: https://www.cloudflare.com/ips/  (refresh occasionally)
set_real_ip_from 173.245.48.0/20;
set_real_ip_from 103.21.244.0/22;
set_real_ip_from 103.22.200.0/22;
set_real_ip_from 103.31.4.0/22;
set_real_ip_from 141.101.64.0/18;
set_real_ip_from 108.162.192.0/18;
set_real_ip_from 190.93.240.0/20;
set_real_ip_from 188.114.96.0/20;
set_real_ip_from 197.234.240.0/22;
set_real_ip_from 198.41.128.0/17;
set_real_ip_from 162.158.0.0/15;
set_real_ip_from 104.16.0.0/13;
set_real_ip_from 104.24.0.0/14;
set_real_ip_from 172.64.0.0/13;
set_real_ip_from 131.0.72.0/22;
set_real_ip_from 2400:cb00::/32;
set_real_ip_from 2606:4700::/32;
set_real_ip_from 2803:f800::/32;
set_real_ip_from 2405:b500::/32;
set_real_ip_from 2405:8100::/32;
set_real_ip_from 2a06:98c0::/29;
set_real_ip_from 2c0f:f248::/32;
real_ip_header CF-Connecting-IP;
```

### 3.4 Don't cache or rewrite at nginx

We want **Cloudflare** to be the cache, and the origin (tileserver) already sets correct
`Cache-Control`. Make sure nginx isn't adding `expires`/`Cache-Control`/`no-store` on the
`/tiles` or `/api/places` locations, and isn't `proxy_hide_header Cache-Control`. If you have a
generic `add_header Cache-Control ...` anywhere on these paths, scope it out.

### 3.5 Lock the origin to Cloudflare — **DO THIS LAST (after §5 verify)**

Once it works through Cloudflare, prevent anyone from hitting `134.209.202.236` directly
(bypassing cache + exposing the origin). Pick one:

- **DO Cloud Firewall / UFW**: allow inbound `443` (and `80` if you keep HTTP-01 renewals) **only
  from the Cloudflare IP ranges above**; keep `22` (SSH) open to your admin IP. Example UFW:
  ```bash
  # for each CF range:
  sudo ufw allow proto tcp from 173.245.48.0/20 to any port 443
  # ... (repeat for every range in §3.3) ...
  sudo ufw deny 443
  sudo ufw allow 22         # keep SSH
  ```
- **Or Cloudflare Authenticated Origin Pulls** (stronger): CF presents a client cert to the
  droplet; nginx verifies it (`ssl_client_certificate` + `ssl_verify_client on`). Then even
  someone who knows the IP can't talk to nginx without CF's cert.

> ⚠️ If you keep Let's Encrypt **HTTP-01** renewal, leave port 80 `/.well-known/acme-challenge/`
> reachable, or move renewals to **DNS-01** (Cloudflare API token) so you can fully close 80.
> Using a **Cloudflare Origin CA cert (§3.1)** sidesteps this entirely.

---

## 4. Cloudflare dashboard (zone `sandring.no`)

### 4.1 SSL/TLS mode
SSL/TLS → **Overview** → set encryption mode to **Full (strict)**.
(Requires the valid origin cert from §3.1. "Flexible" would break HTTPS-origin and is unsafe —
don't use it.)

### 4.2 Turn on the orange cloud (the cutover)
DNS → Records:
- `kart-api` (A → `134.209.202.236`): click the grey cloud → **Proxied (orange)**.
- `kart` (A → `134.209.202.236`): same → **Proxied (orange)**.

Propagates in seconds. Verify with §5.1 immediately; if anything's wrong, toggle back to grey.

### 4.3 Cache Rules
Caching → **Cache Rules** → **Create rule** (rules run top-down; create these, leave everything
else to default = not cached):

**Rule 1 — Tiles (honour origin TTL):**
- Name: `tiles-cache`
- When incoming requests match: **URI Path** `starts with` `/tiles/`
- Then:
  - Cache eligibility → **Eligible for cache**
  - Edge TTL → **Use cache-control header if present, bypass cache if not** (origin already sends
    `max-age=86400` etc.)
  - Browser TTL → **Respect origin**
  - (Cache key: defaults are fine — tile paths are unique by `/z/x/y`, no query string.)

**Rule 2 — Places search/reverse (set an edge TTL, since the API sends no Cache-Control yet):**
- Name: `places-search-cache`
- When: **URI Path** `starts with` `/api/places/search` **OR** `starts with` `/api/places/reverse`
- Then:
  - Cache eligibility → **Eligible for cache**
  - Edge TTL → **Override origin**, **300 seconds** (data refreshes weekly; 5 min is safe and
    keeps autocomplete snappy)
  - Cache key → **include query string** (it's the default; this is what makes `?q=oslo` distinct
    from `?q=bergen`). Optionally normalise: include only `q`, `lat`, `lon`, `limit`.

> Do **NOT** add a rule that caches `/api/route` (SSE stream + per-pair uniqueness) or any other
> `/api/*` (auth/sync/sharing are user-specific). Default behaviour already bypasses them.

### 4.4 Tiered Cache (free, recommended)
Caching → **Tiered Cache** → enable **Smart Tiered Cache Topology**. Makes one upper-tier PoP
pull from your origin and fan out to other PoPs — fewer trips across the tunnel to home.

### 4.5 (Optional) Email Routing for `sigmund@sandring.no`
The Android settings screen now shows `sigmund@sandring.no`. To make it a real inbox:
Email → **Email Routing** → enable → add a route `sigmund@sandring.no` → forward to your Gmail.
(Free. Adds the MX/TXT records automatically.) If you don't want that address, change it in
`apps/android/.../SettingsScreen.kt`.

---

## 5. Verify

```bash
# 5.1 Proxied? (expect: server: cloudflare, a cf-ray header)
curl -sI "https://kart-api.sandring.no/api/places/search?q=Oslo" | grep -iE 'server:|cf-ray'

# 5.2 Tiles cache: first call MISS, second HIT
curl -sI "https://kart-api.sandring.no/tiles/v1/basemap/13/4341/2381.mvt" | grep -i cf-cache-status
curl -sI "https://kart-api.sandring.no/tiles/v1/basemap/13/4341/2381.mvt" | grep -i cf-cache-status   # expect HIT

# 5.3 Search cache: second identical query HIT
curl -sI "https://kart-api.sandring.no/api/places/search?q=Oslo" | grep -i cf-cache-status   # MISS
curl -sI "https://kart-api.sandring.no/api/places/search?q=Oslo" | grep -i cf-cache-status   # HIT

# 5.4 Latency: warm should drop from ~139ms toward ~5ms (cached) / ~40-60ms (uncached)
for i in 1 2 3; do curl -s -o /dev/null -w '%{time_total}\n' "https://kart-api.sandring.no/api/places/search?q=Oslo"; done

# 5.5 Dynamic still bypasses (expect: cf-cache-status: DYNAMIC or BYPASS, never HIT)
curl -sI "https://kart-api.sandring.no/api/route/..." | grep -i cf-cache-status

# 5.6 Origin still works directly (until you lock §3.5)
curl -sI --resolve kart-api.sandring.no:443:134.209.202.236 "https://kart-api.sandring.no/healthz"
```

---

## 6. Cache invalidation on a basemap rebuild

Tile URLs (`/tiles/v1/basemap/{z}/{x}/{y}.mvt`) carry **no version in the path**, so after you
rebuild/re-provision the N50 basemap the edge will serve stale tiles until TTL (24 h) expires.
On the **Free plan** prefix-purge isn't available, so:

- **Now:** after a basemap rebuild, Caching → Configuration → **Purge Cache** → **Purge
  Everything** (rare event — basemap changes slowly). The 24 h `stale-while-revalidate` softens
  it anyway.
- **Durable (small code change, optional):** put the data version in the tile path
  (`/tiles/v1/basemap/{DATA_VERSION}/{z}/{x}/{y}.mvt`) so a rebuild changes the URL space and you
  can cache `immutable` forever with no purge. (This mirrors what the old R2 worker did with
  `DATA_VERSION`.) Ask and I'll wire it.

---

## 7. Optional follow-ups (not required for the win)

- **Web app uses `/tiles`**: the React app still fetches tiles at `${API_BASE}/v1/...` directly.
  Repoint its tile templates to `${API_BASE}/tiles/v1/...` so browser tile traffic also rides the
  edge cache (style.json-consuming clients already get `/tiles` for free). Small `config.ts` /
  template change — ask and I'll do it.
- **`Cache-Control` on search/reverse in code**: add `public, max-age=300` to the `search` /
  `reverse` controller actions in `apps/api/.../PlacesController.cs` so the cache policy lives in
  the codebase and Rule 2 can use "respect origin" instead of an override. ~2 lines.
- **Argo Smart Routing** ($5/mo + usage): speeds the CF→origin leg over the tunnel. Only worth it
  if the CF↔droplet path turns out slow after the above.

---

## Cost

| Item | Plan | Cost |
|---|---|---|
| Proxy `kart-api`/`kart` + Cache Rules + Tiered Cache + Email Routing | **Cloudflare Free** | **$0/mo** |
| Droplet changes (nginx keepalive, real-IP, firewall) | — | $0 |
| (Optional) Argo Smart Routing | add-on | $5/mo + $0.10/GB |

The whole performance fix is **$0** on the Free plan.
