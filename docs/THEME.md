# Theme Architecture

This document describes the wizard theme's structure, template hierarchy, and asset processing.

## Theme Hierarchy

The wizard theme uses a **layered architecture** that overlays the poison base theme:

```
Request for template
        ↓
themes/wizard/layouts/   (check first)
        ↓
themes/wizard/poison/layouts/   (fallback)
```

Only files that need customization exist in `themes/wizard/`. Everything else falls through to poison.

## Directory Structure

```
themes/wizard/
├── layouts/
│   ├── _default/
│   │   ├── baseof.html      # Base document structure
│   │   ├── single.html      # Single page/post template
│   │   └── list.html        # Archive/list pages
│   ├── index.html           # Homepage
│   └── partials/
│       ├── head/
│       │   └── head.html    # <head> contents
│       ├── sidebar/
│       │   ├── title.html   # Site branding
│       │   └── socials.html # Social links
│       ├── post/
│       │   ├── meta.html    # Post metadata (date, tags)
│       │   └── info.html    # Post info
│       ├── navigation.html  # Main nav menu
│       └── light_dark.html  # Theme toggle button
├── assets/
│   ├── css/                 # Stylesheets (see docs/CSS.md)
│   └── js/                  # JavaScript files
├── static/                  # Static files (copied as-is)
├── poison/                  # Base theme (submodule)
└── theme.toml              # Theme metadata
```

## Template Hierarchy

### Base Template: `baseof.html`

The foundation for all pages. Defines the HTML structure:

```html
<!DOCTYPE html>
<html lang="{{ .Site.LanguageCode }}">
    <head>
        <!-- Meta tags -->
        <!-- CSS with fingerprinting + SRI -->
        <!-- JavaScript with fingerprinting + SRI -->
    </head>
    <body>
        <div class="container">
            <header class="site-header">
                <div class="header-image"></div>
                <div class="header-content">
                    {{ partial "sidebar/title.html" . }}
                    {{ partial "light_dark.html" . }}
                </div>
                <nav class="main-nav">
                    {{ partial "navigation.html" . }}
                </nav>
            </header>

            <main id="content">
                {{ block "main" . }}{{ end }}
            </main>

            <footer class="site-footer">
                {{ partial "sidebar/socials.html" . }}
                <div id="metrics-dashboard"></div>
                <div id="memory-game"></div>
                <button id="reset-game">Reset Game</button>
                <p class="copyright">...</p>
            </footer>
        </div>
    </body>
</html>
```

### Content Templates

| Template | Purpose | Block |
|----------|---------|-------|
| `index.html` | Homepage | `{{ define "main" }}` |
| `single.html` | Individual post/page | `{{ define "main" }}` |
| `list.html` | Archive/tag/category pages | `{{ define "main" }}` |

Each template defines a `main` block that fills `{{ block "main" . }}` in baseof.html.

### Partials

Reusable template fragments:

| Partial | Purpose |
|---------|---------|
| `sidebar/title.html` | Site title/brand display |
| `sidebar/socials.html` | Social media links |
| `navigation.html` | Main navigation menu |
| `light_dark.html` | Theme toggle button (sun/moon) |
| `post/meta.html` | Post date, reading time, tags |
| `post/info.html` | Additional post information |

## Asset Processing

Hugo Pipes process assets at build time for optimization and security.

### CSS Processing

```go-html-template
{{ $style := resources.Get "css/style.css" }}
{{ $style = $style | resources.Minify | resources.Fingerprint }}
<link rel="stylesheet"
      href="{{ $style.RelPermalink }}"
      integrity="{{ $style.Data.Integrity }}">
```

**Pipeline:**
1. `resources.Get` - Load from `assets/css/`
2. `resources.Minify` - Remove whitespace, optimize
3. `resources.Fingerprint` - Add content hash to filename

**Output:** `/css/style.min.abc123.css` with SRI hash

### JavaScript Processing

```go-html-template
{{ $lightDark := resources.Get "js/light_dark.js" }}
{{ $lightDark = $lightDark | resources.Minify | resources.Fingerprint }}
<script src="{{ $lightDark.RelPermalink }}"
        integrity="{{ $lightDark.Data.Integrity }}"></script>
```

### Lazy-Loading Large Libraries

Chart.js is exposed as a URL for on-demand loading:

```go-html-template
{{ $chartJS := resources.Get "js/chart.min.js" }}
{{ $chartJS = $chartJS | resources.Fingerprint }}
<script>
    window.CHART_JS_URL = "{{ $chartJS.RelPermalink }}";
    window.CHART_JS_INTEGRITY = "{{ $chartJS.Data.Integrity }}";
</script>
```

JavaScript can then load it dynamically:
```javascript
const script = document.createElement('script');
script.src = window.CHART_JS_URL;
script.integrity = window.CHART_JS_INTEGRITY;
document.head.appendChild(script);
```

## JavaScript Files

| File | Purpose | Size |
|------|---------|------|
| `pixel-scale.js` | DPR-based pixel art scaling | ~20 lines |
| `light_dark.js` | Theme toggle, localStorage persistence | ~30 lines |
| `memory-game.js` | Footer card matching game | ~200 lines |
| `metrics-dashboard.js` | Real-time server metrics | ~150 lines |
| `chart.min.js` | Chart.js library (lazy-loaded) | ~200KB |
| `mermaid.esm.min.mjs` | Diagram rendering (lazy-loaded) | ~1MB |

## Site Configuration

Navigation and social links are configured in `hugo.toml`:

```toml
[params]
    brand = "sven.guru"
    description = "Thoughts about tech"

    # Navigation menu
    menu = [
        {Name = "About", URL = "/about/", HasChildren = false},
        {Name = "Posts", URL = "/posts/", HasChildren = false},
    ]

    # Social links (shown in footer)
    email_url = "mailto:..."
    github_url = "https://github.com/..."
    linkedin_url = "https://linkedin.com/in/..."
    mastodon_url = "https://..."
    rss_icon = true
```

## Common Modifications

### Adding a New CSS File

1. Create `themes/wizard/assets/css/new-styles.css`
2. Add to `baseof.html`:
   ```go-html-template
   {{ $newStyles := resources.Get "css/new-styles.css" }}
   {{ $newStyles = $newStyles | resources.Minify | resources.Fingerprint }}
   <link rel="stylesheet" href="{{ $newStyles.RelPermalink }}"
         integrity="{{ $newStyles.Data.Integrity }}">
   ```

### Adding a New JavaScript File

1. Create `themes/wizard/assets/js/new-script.js`
2. Add to `baseof.html`:
   ```go-html-template
   {{ $newScript := resources.Get "js/new-script.js" }}
   {{ $newScript = $newScript | resources.Minify | resources.Fingerprint }}
   <script src="{{ $newScript.RelPermalink }}"
           integrity="{{ $newScript.Data.Integrity }}"></script>
   ```

### Adding a New Partial

1. Create `themes/wizard/layouts/partials/my-partial.html`
2. Include in templates: `{{ partial "my-partial.html" . }}`

### Adding a New Page Template

For a special page type (e.g., gallery):

1. Create `themes/wizard/layouts/gallery/single.html`:
   ```go-html-template
   {{ define "main" }}
   <div class="gallery">
       {{ .Content }}
       <!-- Custom gallery markup -->
   </div>
   {{ end }}
   ```

2. Set `type: gallery` in content front matter

## Poison Base Theme

The poison theme in `themes/wizard/poison/` provides fallback templates. Key files:

- `layouts/_default/` - Default templates
- `assets/` - Base styles (mostly overridden)
- `static/` - Fonts and base assets

**Generally don't modify poison directly.** Instead, override in wizard.

## Subresource Integrity (SRI)

All assets include SRI hashes for security:

```html
<link rel="stylesheet"
      href="/css/style.min.abc123.css"
      integrity="sha256-...">
```

This prevents CDN compromise or MITM attacks from injecting malicious code. The browser verifies the hash matches before executing.
