# Lighthouse Audit — Remediation Plan

Source: Lighthouse 12.6.0, mobile, run 2026-04-22 against `https://sven.guru/`.

Original category scores (production, mobile):

| Category       | Score |
|----------------|-------|
| Performance    | 83    |
| Accessibility  | 85    |
| Best Practices | 100   |
| SEO            | 92    |

Local run 2026-04-23 (after tasks 1, 2, 3-partial, 4, 5, 6):

| Category       | Mobile | Desktop |
|----------------|--------|---------|
| Performance    | 88     | 100     |
| Accessibility  | 100    | 100     |
| Best Practices | 96     | 96      |
| SEO            | 100    | 100     |

Original failing Core Web Vital: **CLS = 0.225** — now 0 (fixed).
LCP on throttled localhost: 3.9s mobile / 0.8s desktop. Re-audit production to get a real number.

---

## In-repo tasks (this repo: `~/development/homepage`)

Ordered by impact. Each task is independent unless noted.

### 1. Add alt text to homepage image ✅ DONE (43e5758, 9afd9a4)
- **Audit:** `image-alt` (a11y, SEO), score 0, severity critical.
- **Problem:** `/ox-hugo/2026-01-20_19-52-46_screenshot.png` is rendered in the homepage without an `alt` attribute.
  - Selector: `div.about-text > div.homepage-content > figure.homepage-image > img`
  - Snippet: `<img src="/ox-hugo/2026-01-20_19-52-46_screenshot.png">`
- **Fix:** Edit the source in `content-org/all-pages.org` (NOT the generated `/content/` markdown). Find the Org link to `2026-01-20_19-52-46_screenshot.png` on the homepage section and add a descriptive alt text. Re-export with `C-c C-e`.
- **Acceptance:** `<img>` has non-empty `alt` attribute. Lighthouse `image-alt` passes.

### 2. Reduce CLS from web font loading ✅ DONE (931a26f) — CLS now 0
- **Audit:** `cumulative-layout-shift` = 0.225, `layout-shifts`, `cls-culprits-insight`. Needs improvement.
- **Problem:** The three self-hosted fonts (`Phosphor.woff2`, `thaleah-fat.woff2`, `monaspace-argon-variable.woff2`) swap in after first paint and reflow the entire `<main>` element (score 0.225). Lighthouse attributes all three as causes.
- **Files:**
  - `themes/wizard/layouts/_default/baseof.html` — `<head>`
  - `themes/wizard/assets/css/style.css` — `@font-face` rules (ThaleahFat, Monaspace Argon)
  - `themes/wizard/assets/css/phosphor.css` — Phosphor `@font-face`
- **Fix options** (pick one or combine):
  1. **Preload** the three woff2 files with `<link rel="preload" as="font" type="font/woff2" crossorigin>` in `baseof.html` so they're discoverable before CSS parsing.
  2. **Metric overrides** on `@font-face`: add `size-adjust`, `ascent-override`, `descent-override`, `line-gap-override` tuned against a system-font fallback to make the swap non-shifting. Tools: <https://meowni.ca/font-style-matcher/> or Capsize.
  3. Mark Monaspace Argon with `font-display: optional` (body text still readable in fallback, avoids swap CLS). Keep ThaleahFat as `swap` since headings are critical to the pixel aesthetic.
- **Acceptance:** CLS ≤ 0.1 in mobile Lighthouse. No visible jump when fonts finish loading.

### 3. Optimize LCP hero image (~196 KiB savings) ⚠️ PARTIAL (8e18f1d)

**Done:** smaller mobile source served via `srcset`.
**Still open:** `fetchpriority="high"` on the visible hero, re-encode at lower quality, and the follow-up about shipping only one theme's hero.
- **Audits:** `largest-contentful-paint` 2.9s, `image-delivery-insight` (196 KiB savings), `lcp-discovery-insight` (fetchpriority not applied), `prioritize-lcp-image`.
- **Problem:** Hero backgrounds `/images/header-tower-light.webp` (145 KiB) and `/images/header-tower-dark.webp` (109 KiB) are the LCP element. Lighthouse says:
  - 116 KiB savings on light, 80 KiB on dark via stronger compression
  - No `fetchpriority="high"` applied
  - Same asset served regardless of viewport
- **Files:**
  - Template with `<img class="hero-bg-light">` / `hero-bg-dark` — find with `rg 'hero-bg-light' themes/wizard/layouts`
  - `static/images/header-tower-*.webp`
- **Fix:**
  1. Add `fetchpriority="high"` to the currently-visible hero `<img>` (and keep `loading="eager"`, which is default for visible images).
  2. Re-encode both WebPs at a lower quality (try `cwebp -q 70` — pixel art survives aggressive compression because of `image-rendering: pixelated`).
  3. Optionally provide a smaller mobile source via `srcset` + `sizes` (e.g. 412w and 1024w variants).
- **Acceptance:** LCP < 2.5s, hero transfers ~50 KiB or less.
- **Follow-up (not yet addressed):** Both `<img class="hero-bg-light">` and `<img class="hero-bg-dark">` sit in the DOM so the theme toggle can crossfade between them, but this means every visitor downloads *both* hero variants even though only one is ever visible. Options to explore:
  1. Render only the theme-matching `<img>` from a server-side hint (cookie set by the theme toggle JS) and omit the other — the first paint is still correct since the initial theme defaults to dark. The toggle then either swaps `src` on the existing `<img>` or inserts the second one lazily on first toggle.
  2. Use a single `<img>` and swap its `src`/`srcset` in the theme toggle handler. Fade via opacity on a wrapper.
  3. Keep the crossfade, but mark the non-initial `<img>` as `loading="lazy"` + `fetchpriority="low"` so it only loads when the theme actually flips.
  Pick based on how much the crossfade matters vs. doubling hero bytes on every load.

### 4. Fix muted text contrast (light theme) ✅ DONE (1b79eff)
- **Audit:** `color-contrast` fails, severity serious. WCAG AA requires 4.5:1 for body text.
- **Problem:** `--color-text-muted: #9a8672` on `--color-bg-base: #f3ece0` = 2.96:1. Affects:
  - Post card meta (date, read time)
  - Footer metric labels (Req/s, Latency, Viewers)
  - Footer tagline (`// No cookies. No tracking. Just vibes.`)
  - Footer info (© 2026…, Powered by Rust + Hugo)
- **File:** `themes/wizard/assets/css/style.css`, `[data-theme="light"]` block (~line 94).
- **Fix:** Darken `--color-text-muted`. `#6b5a44` hits ~5.1:1 on `#f3ece0` and still reads as muted brown. Verify the same token isn't relied on in dark theme (it isn't — dark theme defines its own).
- **Acceptance:** All `color-contrast` items pass. Muted text still visually distinct from primary.

### 5. Fix inline link contrast in body text ✅ DONE (6231199)
- **Audit:** `link-in-text-block` fails, severity serious.
- **Problem:** Inline links in the homepage intro paragraph (`posts`, `about me`) use `--color-link: #1a7a4c` against body text `#3b2f20` — 2.43:1 and no underline. WCAG requires 3:1 against surrounding text OR a non-color distinction.
- **File:** `themes/wizard/assets/css/style.css`.
- **Fix:** Add `text-decoration: underline` to inline links inside prose. Easiest selector: underline links inside `.homepage-content p`, `.post-content p`, `.post-summary-text` (but not `.post-card` or nav/tag/button styled links). Keep the teal color for visual identity.
- **Acceptance:** Inline prose links have a visible underline. Lighthouse `link-in-text-block` passes.

### 6. Fix reset-button contrast and font size ✅ DONE (1b79eff)
- **Audits:** `color-contrast` (4.32:1 fails AA for <14pt), `font-size` (8px < 12px).
- **Problem:** `.reset-button` in `memory-game.css`:
  - `color: #2d7a82` on `background: #e8f0f8` = 4.32:1 (fails for 8px text, needs 4.5:1)
  - `font-size: 0.5rem` = 8px, too small for SEO font-size audit
- **File:** `themes/wizard/assets/css/memory-game.css` (~line 201, `.reset-button`).
- **Fix:** Bump `font-size` to at least `0.75rem` (12px). Darken `color` to `#1e5a60` or similar for ~6:1 contrast. Keep the chunky pixel aesthetic.
- **Acceptance:** Button remains visually on-theme, passes both audits.

### 7. Fix BFCache blocked by WebSocket
- **Audit:** `bf-cache` fails — "Pages with WebSocket cannot enter back/forward cache."
- **Problem:** Metrics dashboard opens a WebSocket for live server metrics; an open socket blocks bfcache so back/forward navigation re-runs the full page load.
- **File:** `themes/wizard/assets/js/metrics-dashboard.js`.
- **Fix:** Close the WebSocket on `pagehide` and reopen on `pageshow` (when `event.persisted` is true, the page came from bfcache). Sketch:
  ```js
  window.addEventListener('pagehide', () => { ws && ws.close(); });
  window.addEventListener('pageshow', (e) => { if (e.persisted) reconnect(); });
  ```
- **Acceptance:** `bf-cache` audit passes. Metrics still update on first load and resume after back-navigation.

### 8. Bump small font sizes ⚠️ NOT CURRENTLY FAILING

Local Lighthouse 12.6 run no longer surfaces the `font-size` audit at all
(mobile SEO is 100). Re-confirm against production before doing this — the
`.footer-tagline` and `.tag` are still at 0.7rem, but Lighthouse may have
dropped or relaxed the audit.
- **Audit:** `font-size` — 1.7% of text is <12px.
- **Problem:** Three selectors render below 12px:
  - `.footer-tagline` — 11.2px (0.7rem)
  - `.tag` — 11.2px (0.7rem)
  - `.reset-button` — 8px (already handled in task 6)
- **File:** `themes/wizard/assets/css/style.css`.
- **Fix:** Bump `.footer-tagline` and `.tag` `font-size` to `0.75rem` (12px).
- **Acceptance:** `font-size` audit shows 100% legible text.

---

## Out-of-repo tasks (handover — `~/nixos-config`)

These live in the NixOS config repo (`modules/homepage.nix` or a dedicated Caddy module). Current deployment serves the homepage via Caddy in front of a Rust server on localhost:8080.

### A. Long cache TTL for static assets
- **Audit:** `uses-long-cache-ttl`, `cache-insight` — 321 KiB potentially saved on repeat visits. Est LCP savings 1.45s on repeat views.
- **Current state:** All static assets return `Cache-Control: public, max-age=3600` (1 hour). Lighthouse calls this too short. Affected files:
  - Fonts: `Phosphor.woff2`, `monaspace-argon-variable.woff2`, `thaleah-fat.woff2`
  - Images: `header-tower-light.webp`, `header-tower-dark.webp`, ox-hugo screenshots
- **Recommended Caddy config:**
  ```caddy
  # Fingerprinted CSS/JS (Hugo pipes add a hash to the filename) — safe to cache forever
  @fingerprinted path_regexp \.[0-9a-f]{32,}\.(css|js)$
  header @fingerprinted Cache-Control "public, max-age=31536000, immutable"

  # Fonts, images — cache long, but allow revalidation
  @assets path_regexp \.(woff2?|webp|png|jpg|jpeg|svg|ico)$
  header @assets Cache-Control "public, max-age=2592000"  # 30 days

  # HTML — short TTL so content updates propagate
  @html path_regexp \.html$|/$
  header @html Cache-Control "public, max-age=300, must-revalidate"  # 5 min
  ```
- **Note:** Hugo's resource fingerprinting means CSS/JS URLs change on every content change, so `immutable` with a long TTL is safe. The Rust server may also need to honour conditional `If-Modified-Since` / `ETag` — verify with `curl -I`.
- **Acceptance:** `curl -I https://sven.guru/fonts/monaspace-argon-variable.woff2` returns `Cache-Control: public, max-age=…` with a large value.

### B. Security headers
- **Audits:** `csp-xss`, `has-hsts`, `origin-isolation`, `clickjacking-mitigation` — all currently "no header found" (informative, not scored, but flagged as High severity).
- **Recommended Caddy config:**
  ```caddy
  header {
      # HSTS — start with a lower max-age, bump after confirming no issues
      Strict-Transport-Security "max-age=31536000; includeSubDomains"

      # Clickjacking — page is not meant to be embedded
      X-Frame-Options "DENY"
      # (Equivalent CSP directive: frame-ancestors 'none')

      # COOP — isolate top-level window
      Cross-Origin-Opener-Policy "same-origin"

      # Prevent MIME sniffing (not scored but cheap)
      X-Content-Type-Options "nosniff"

      # Referrer — conservative default
      Referrer-Policy "strict-origin-when-cross-origin"

      # CSP — site is static, no third-party JS. Self-hosted fonts/images only.
      # 'unsafe-inline' is needed for the inline window.CHART_JS_URL script in baseof.html;
      # consider moving that to an external file to tighten this further.
      Content-Security-Policy "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self'; img-src 'self' data:; font-src 'self'; connect-src 'self' wss://sven.guru; frame-ancestors 'none'; base-uri 'self'; form-action 'self'"
  }
  ```
- **Notes:**
  - The metrics dashboard uses a WebSocket — whitelist `wss://sven.guru` in `connect-src`.
  - Before enabling HSTS with a large `max-age`, test with `max-age=300` first and confirm the cert auto-renews cleanly.
  - The inline `window.CHART_JS_URL = …` script in `baseof.html` is the only inline script today. If CSP reports fire, either move it to an external file with a `nonce`/`hash`, or leave `'unsafe-inline'` for `script-src`.
- **Acceptance:** `curl -I https://sven.guru/` returns all five headers. Lighthouse security audits flip from "No header found" to passing. Site still loads in both themes; memory game + metrics dashboard still work.

---

## Suggested order of execution

1. ~~**Quick wins first** (tasks 1, 8, 6, 5)~~ — done (1, 5, 6). Task 8 no longer flagged.
2. ~~**CLS fix** (task 2)~~ — done.
3. **LCP image** (task 3) — still open: `fetchpriority`, re-encode, theme-matched hero.
4. **BFCache** (task 7) — still open.
5. Hand over A and B to the nixos-config agent.
