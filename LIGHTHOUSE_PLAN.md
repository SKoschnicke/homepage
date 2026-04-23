# Lighthouse Audit — Remediation Plan

Original source: Lighthouse 12.6.0, mobile, run 2026-04-22 against `https://sven.guru/`.

## Current status

Local run 2026-04-23 (after a11y + CLS fixes, partial LCP work):

| Category       | Mobile | Desktop |
|----------------|--------|---------|
| Performance    | 88     | 100     |
| Accessibility  | 100    | 100     |
| Best Practices | 96     | 96      |
| SEO            | 100    | 100     |

Localhost runs are under Lighthouse's simulated slow-4G throttling, so
absolute numbers (especially LCP) differ from production. Use local runs for
deltas, re-audit `https://sven.guru/` for real numbers.

A11y and SEO are at 100 — remaining work is performance-related. Most of it
involves the Rust server and/or Caddy in front of it, so the natural next
step is handing off to the nixos-config / server agent.

Runner: `mise run lighthouse` (writes JSON + HTML reports to `.lighthouse/`).

---

## Done (keep for commit trail)

- ✅ **1. Alt text on homepage image** — 43e5758, 9afd9a4
- ✅ **2. CLS from font swap** — 931a26f (CLS 0.225 → 0)
- ✅ **3. LCP partial** — 8e18f1d (smaller mobile hero via `srcset`)
- ✅ **4. Muted text contrast** — 1b79eff (`--color-text-muted` darkened)
- ✅ **5. Inline link underline** — 6231199 (prose links now underlined)
- ✅ **6. Reset button contrast + size** — 1b79eff (`.reset-button`)
- ✅ **8. Small font sizes** — audit no longer surfaced by LH 12.6, likely dropped

---

## Still open

### LCP hero image — finish task 3

Smaller mobile hero already shipped. What's left:

1. **`fetchpriority="high"`** on the currently-visible hero `<img>` (and
   keep `loading="eager"`, which is default for visible images). Find the
   template with `rg 'hero-bg-light' themes/wizard/layouts`.
2. **Re-encode** `static/images/header-tower-{light,dark}.webp` at a lower
   quality. Lighthouse flagged 116 KiB savings on light + 80 KiB on dark.
   Pixel art survives aggressive compression because the CSS uses
   `image-rendering: pixelated`. Try `cwebp -q 70` first.
3. **Ship only one hero**: today both `<img class="hero-bg-light">` and
   `<img class="hero-bg-dark">` are in the DOM so the theme toggle can
   crossfade. That doubles hero bytes on every load. Options:
   1. Render only the theme-matching `<img>` based on a server-side cookie
      set by the theme-toggle JS. First paint is correct since the initial
      theme defaults to dark. Toggle lazily inserts the other variant.
   2. Single `<img>`, swap `src`/`srcset` in the toggle handler. Fade via
      opacity on a wrapper.
   3. Keep the crossfade but mark the non-initial `<img>` as
      `loading="lazy"` + `fetchpriority="low"`.

**Acceptance:** LCP < 2.5s on production Lighthouse, hero transfers ~50 KiB
or less.

Relevant audits: `largest-contentful-paint`, `image-delivery-insight`,
`lcp-discovery-insight`, `prioritize-lcp-image`.

### BFCache + WebSocket — task 7 (DEFERRED)

Lighthouse flags `bf-cache` because the metrics dashboard opens a WebSocket
to `/__metrics__/ws`, and Chrome disqualifies any page that ever opened a
WebSocket from bfcache. The naive fix (close on `pagehide`) doesn't help —
Chrome's bfcache eligibility check looks at whether a WebSocket was *used*,
not whether one is currently open.

Real fix options (all are user-visible tradeoffs):

1. **Gate the socket behind a user gesture** (e.g. clicking "Show more" to
   expand the dashboard). Compact metrics sit at `--` until expansion.
   Loses the "live by default" feel.
2. **Poll via fetch instead of WebSocket.** fetch/XHR don't block bfcache
   if no request is in flight at navigation time. Server endpoint would
   need a regular HTTP `/__metrics__/json` alongside or replacing the WS.
3. **Accept the failure.** `bf-cache` is informational; the live metrics
   feature is the whole point of this site, not worth gutting for a
   synthetic back-nav speedup.

**Decision pending.** Recommend option 3 unless bfcache becomes a scored
category.

### Performance insights that need server work

Three audits are blocked on server/infra changes, not frontend:

- `cache-insight` (0) — short cache TTLs, see handover task A.
- `image-delivery-insight` (0) — overlaps with LCP work + server-side
  Accept-Encoding and Content-Encoding (brotli/zstd if not already on).
- `render-blocking-insight` (0) — stylesheet is render-blocking. Could
  inline critical CSS in `<head>`, or split off above-the-fold CSS.
- `document-latency-insight` (0.5) — TTFB; likely the Rust server or TLS
  handshake. Measure on production, not localhost.
- `network-dependency-tree-insight` (0) — informational, trace what's
  serial vs parallel on first paint.
- `unused-javascript` (0) — Chart.js is the main culprit. It's already
  lazy-loaded only when the dashboard expands, so this is mostly a false
  positive on the first page view. Confirm with the Coverage panel on
  production.

**None of these are pure frontend fixes.** They need coordination with the
Rust server and/or Caddy.

---

## Handover — `~/nixos-config` / Rust server

Current deployment: Caddy in front of a Rust server on localhost:8080.

### A. Long cache TTL for static assets

**Audit:** `uses-long-cache-ttl`, `cache-insight` — 321 KiB potentially
saved on repeat visits, est. LCP savings 1.45s on repeat views.

**Current state:** All static assets return `Cache-Control: public, max-age=3600`.
Affected files:
- Fonts: `Phosphor.woff2`, `monaspace-argon-variable.woff2`, `thaleah-fat.woff2`
- Images: `header-tower-light.webp`, `header-tower-dark.webp`, ox-hugo screenshots

**Decision needed:** which side sets caching headers — Caddy (easy) or the
Rust server (more correct, since the server embeds the assets via
`build.rs` and owns the response)? Probably the Rust server.

Recommended Caddy config if going that route:

```caddy
# Fingerprinted CSS/JS — safe to cache forever
@fingerprinted path_regexp \.[0-9a-f]{32,}\.(css|js)$
header @fingerprinted Cache-Control "public, max-age=31536000, immutable"

# Fonts, images — long cache, allow revalidation
@assets path_regexp \.(woff2?|webp|png|jpg|jpeg|svg|ico)$
header @assets Cache-Control "public, max-age=2592000"  # 30 days

# HTML — short TTL so content updates propagate
@html path_regexp \.html$|/$
header @html Cache-Control "public, max-age=300, must-revalidate"  # 5 min
```

Hugo fingerprints CSS/JS so `immutable` + long TTL is safe on those. Verify
with `curl -I` once deployed.

**Acceptance:** `curl -I https://sven.guru/fonts/monaspace-argon-variable.woff2`
returns `Cache-Control: public, max-age=…` with a large value.

### B. Security headers

**Audits:** `csp-xss`, `has-hsts`, `origin-isolation`, `clickjacking-mitigation`
— all currently "no header found" (informative, not scored).

Recommended Caddy config:

```caddy
header {
    Strict-Transport-Security "max-age=31536000; includeSubDomains"
    X-Frame-Options "DENY"
    Cross-Origin-Opener-Policy "same-origin"
    X-Content-Type-Options "nosniff"
    Referrer-Policy "strict-origin-when-cross-origin"
    # CSP — static site, no third-party JS, self-hosted fonts/images only.
    # 'unsafe-inline' for script-src is needed by the inline
    # window.CHART_JS_URL in baseof.html — consider externalising that.
    Content-Security-Policy "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self'; img-src 'self' data:; font-src 'self'; connect-src 'self' wss://sven.guru; frame-ancestors 'none'; base-uri 'self'; form-action 'self'"
}
```

Notes:
- Metrics WebSocket lives on the same origin → `connect-src 'self' wss://sven.guru`.
- Before enabling HSTS with a year-long `max-age`, try `max-age=300` and
  confirm cert auto-renew still works.
- Only inline script today is `window.CHART_JS_URL = …` in `baseof.html`.
  Either move it to an external file with a nonce/hash, or keep
  `'unsafe-inline'`.

**Acceptance:** `curl -I https://sven.guru/` returns all five headers.
Lighthouse security audits flip from "No header found" to passing. Site
still loads in both themes; memory game + metrics dashboard still work.

### C. Compression

Confirm `Content-Encoding: br` (or at minimum `gzip`) is served for HTML,
CSS, JS, SVG, woff2 (woff2 is already compressed — skip). `curl -H
"Accept-Encoding: br, gzip" -I https://sven.guru/` should show it. If not,
enable in Caddy with `encode zstd br gzip`.

### D. TTFB / document latency

`document-latency-insight` scores 0.5. Investigate on production:

```
curl -w "%{time_starttransfer}\n" -o /dev/null -s https://sven.guru/
```

If TTFB > 200ms, profile the Rust server's first-byte path. Likely
candidates: Hugo-rendered HTML being read from disk, TLS handshake, or
early middleware.

---

## Suggested order

1. Finish **task 3** (hero `fetchpriority` + re-encode + one-hero-per-theme)
   — pure frontend, should land first.
2. Hand off **A, B, C, D** to the nixos-config / server agent.
3. Decide on **task 7** after A-D are in — may be worth a follow-up depending
   on how much the bfcache failure still bothers us.
