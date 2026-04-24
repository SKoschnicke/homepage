# Lighthouse Audit — Remediation Plan

Original source: Lighthouse 12.6.0, mobile, run 2026-04-22 against `https://sven.guru/`.

## Current status

Local run 2026-04-24 against the **Rust server** on :8080 (not `hugo server`
— that hides compression and real cache headers). After fetchpriority fix,
favicon stub, and cache-TTL split:

| Category       | Mobile | Desktop |
|----------------|--------|---------|
| Performance    | 95     | 100     |
| Accessibility  | 100    | 100     |
| Best Practices | 100    | 100     |
| SEO            | 100    | 100     |

Core Web Vitals (mobile / desktop): LCP 2.9s / 0.6s, CLS 0 / 0,
TBT 0ms / 0ms, FCP 1.1s / 0.3s.

Localhost runs are under Lighthouse's simulated slow-4G throttling, so
absolute numbers (especially LCP) differ from production. Use local runs for
deltas, re-audit `https://sven.guru/` for real numbers.

A11y and SEO are at 100 — remaining work is performance-related. Most of it
involves the Rust server and/or Caddy in front of it, so the natural next
step is handing off to the nixos-config / server agent.

Runner: `mise run lighthouse` against a running `mise run dev`
(writes JSON + HTML reports to `.lighthouse/`).

---

## Done (keep for commit trail)

- ✅ **1. Alt text on homepage image** — 43e5758, 9afd9a4
- ✅ **2. CLS from font swap** — 931a26f (CLS 0.225 → 0)
- ✅ **3. LCP partial** — 8e18f1d (smaller mobile hero via `srcset`)
- ✅ **4. Muted text contrast** — 1b79eff (`--color-text-muted` darkened)
- ✅ **5. Inline link underline** — 6231199 (prose links now underlined)
- ✅ **6. Reset button contrast + size** — 1b79eff (`.reset-button`)
- ✅ **8. Small font sizes** — audit no longer surfaced by LH 12.6, likely dropped
- ✅ **3a. Hero fetchpriority** — only the dark (default-theme) hero is
  `fetchpriority="high"`; light is `loading="lazy" fetchpriority="low"`.
  Halved initial hero bytes, +7 mobile perf points.
- ✅ **Lighthouse runner targets Rust server** — `scripts/lighthouse-audit.sh`
  now requires `mise run dev` on :8080, so audits reflect production
  compression + cache semantics instead of the bare Hugo dev server.
- ✅ **Favicon stub** — `<link rel="icon" href="data:,">` in `baseof.html`
  silences Chrome's `/favicon.ico` probe. Closed `errors-in-console`,
  Best Practices 96 → 100.
- ✅ **Cache TTL split** (`server/src/router.rs`) — fingerprinted assets
  stay `immutable`, fonts/images/PDFs get 30 days, everything else (HTML)
  gets 5 min `must-revalidate`. Closed `cache-insight`.

---

## Still open

### LCP hero image

Mobile LCP is still 3.0s on localhost (simulated slow-4G). Re-encoding the
webps at lower quality was tried and rejected — lossy compression ruins the
pixel art even with `image-rendering: pixelated`. Lossless webp is
essentially the current size.

Remaining options, all with tradeoffs:

- **Preload the hero** in `<head>`:
  `<link rel="preload" as="image" href="/images/header-tower-dark.webp" imagesrcset="…" imagesizes="…" fetchpriority="high">`
  Shaves the LCP discovery delay. `lcp-discovery-insight` currently scores 0.
- **Ship only one hero per theme** (was option 1 in the old plan). Would
  need a server-side cookie read by the Rust server — non-trivial since
  assets are statically embedded via `build.rs`. Skip unless we redesign.
- **Smaller source art**. The current light webp is 142 KiB, dark is 107
  KiB at 716×503. Commissioning a smaller/simpler pixel art source is a
  content problem, not a code one.

Likely cheapest next win: preload + confirm on production numbers.

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

### Remaining sub-1.0 audits

- `largest-contentful-paint` (0.8 mobile / 0.99 desktop) — hero is still
  the LCP element. Bytes can't shrink without quality loss. See LCP
  section above; `rel="preload"` may close the gap.
- `image-delivery-insight` (0) — same hero, same constraint.
- `lcp-discovery-insight` (0) — browser discovers the hero late in parse.
  Closed by `<link rel="preload" as="image" imagesrcset=…>` for the dark
  hero. Try this.
- `render-blocking-insight` (0) — every fingerprinted CSS/JS in `<head>`
  blocks paint. Individual wastedMs is small (150–300ms each). Full fix
  is critical-CSS inlining, which is a larger refactor. Defer until
  production LCP confirms it matters.
- `network-dependency-tree-insight` (0) — informational.
- `bf-cache` (0) — deferred, see dedicated section above.

---

## Handover — `server/src/router.rs` + `~/nixos-config`

Current deployment: Caddy in front of a Rust server on localhost:8080. The
Rust server already pre-compresses assets at build time (brotli + gzip,
content-negotiated per request) and already sends
`Cache-Control: public, max-age=31536000, immutable` for Hugo-fingerprinted
files. Compression (task C) is therefore **done**.

What remains:

### A. Cache TTL split — DONE locally, needs deploy

`server/src/router.rs::serve_asset` now has three tiers:

- Fingerprinted (`*.min.HASH.ext`): `max-age=31536000, immutable`
- Long-lived (woff/woff2/webp/png/jpg/jpeg/gif/svg/ico/pdf): 30 days
- Everything else (HTML): `max-age=300, must-revalidate`

Local Lighthouse confirms `cache-insight` closed. **`mise run deploy`** to
push the binary to palanthas.

**Production acceptance:** `curl -I
https://sven.guru/fonts/monaspace-argon-variable.woff2` returns
`Cache-Control: public, max-age=2592000`.

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

### C. Compression — DONE

`server/build.rs` pre-compresses HTML/CSS/JS/XML into gzip + brotli
variants, and `router.rs::serve_asset` negotiates via `Accept-Encoding`.
No action needed.

### D. TTFB / document latency

`document-latency-insight` was 0.5 on the old audit against production.
Recheck on production after A+B:

```
curl -w "%{time_starttransfer}\n" -o /dev/null -s https://sven.guru/
```

If TTFB > 200ms, the likely culprits are TLS handshake (Caddy) or the Rust
server's initial handler latency. The server is already serving from a
static `HashMap` with no syscalls — not much left to optimise at the
handler level.

---

## Suggested order

1. **Deploy** (`mise run deploy`) — ship the favicon + TTL split to prod.
2. **Task B** (Caddy security headers): handoff to `~/nixos-config`.
3. **Re-audit against https://sven.guru/** for real numbers. Localhost
   throttled runs may be misleading for LCP in particular.
4. **LCP preload** (`<link rel="preload" as="image" ...>` for the dark
   hero): cheap, may close `lcp-discovery-insight` and nudge LCP.
5. **D** (TTFB): only if production numbers show it still blocking.
6. Decide on **task 7** (bfcache) last — informational, likely accept
   the failure.
7. **Render-blocking CSS**: last resort, biggest refactor, smallest
   real-world gain.
