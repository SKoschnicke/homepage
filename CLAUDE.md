# CLAUDE.md

Instructions for AI assistants working with this Hugo-based personal homepage.

## Quick Reference

| Task | Action |
|------|--------|
| Edit content | Modify `content-org/all-pages.org`, NOT `/content/*.md` |
| Run dev server | `hugo server -D` |
| Build site | `hugo --minify` |
| Modify styles | Edit `themes/wizard/assets/css/*.css` |
| Modify templates | Edit `themes/wizard/layouts/**/*.html` |
| Full build (Hugo + Rust) | `mise run build` |
| Build for deploy (aarch64) | `mise run build aarch64` |
| Deploy to VPS | `mise run deploy` |

## Critical Rules

1. **Content is Org-mode**: All content lives in `content-org/all-pages.org`. The `/content/` directory is auto-generated - never edit it directly.

2. **Theme is site-specific**: The wizard theme in `themes/wizard/` is meant to be modified directly. It overlays the poison base theme.

3. **Always test both themes**: Light and dark modes share colors but invert backgrounds. Test both when changing styles.

4. **Responsive required**: Test mobile (2x pixel scaling) AND desktop (4x pixel scaling).

5. **Self-hosted assets**: All fonts and JS libraries are self-hosted. No CDN dependencies.

## Project Structure

```
content-org/all-pages.org  # SOURCE OF TRUTH - edit content here
content/                   # Generated (do not edit)
themes/wizard/             # Custom theme (modify directly)
  layouts/                 # Hugo templates
  assets/css/              # Stylesheets
  assets/js/               # JavaScript
  poison/                  # Base theme (rarely touched)
static/images/             # Static assets
hugo.toml                  # Site configuration
flake.nix                  # Nix dev shell (Rust + Hugo + cross-compile toolchain)
.mise.toml                 # Task runner (build, dev, deploy)
server/                    # Rust web server (see server/README.md)
docs/                      # Detailed documentation
```

## Documentation Index

Detailed documentation for specific aspects of the project:

- **[docs/CONTENT.md](docs/CONTENT.md)** - Content workflow, Org-mode structure, front matter
- **[docs/THEME.md](docs/THEME.md)** - Theme architecture, templates, Hugo pipes
- **[docs/CSS.md](docs/CSS.md)** - Design system, colors, typography, responsive design
- **[server/README.md](server/README.md)** - Rust server, VPS deployment, cross-compilation

## Common Tasks

### Adding a New Blog Post

1. Open `content-org/all-pages.org` in Emacs
2. Add a new heading under `* Posts`:
   ```org
   ** DONE Post Title                                              :tag1:tag2:
   :PROPERTIES:
   :EXPORT_FILE_NAME: post-slug
   :END:

   Post content here...
   ```
3. Export with `C-c C-e` (or your ox-hugo export binding)
4. Hugo will pick up the generated markdown

### Modifying Styles

1. Edit files in `themes/wizard/assets/css/`:
   - `style.css` - Main styles, CSS variables, layout
   - `header.css` - Header image, navigation
   - `memory-game.css` - Footer card game
2. Use CSS variables for colors (defined in `:root` and `[data-theme="dark"]`)
3. Test both light and dark themes
4. Hugo automatically minifies and fingerprints on build

### Changing Templates

1. Edit files in `themes/wizard/layouts/`:
   - `_default/baseof.html` - Base document structure
   - `_default/single.html` - Single page/post
   - `_default/list.html` - Archive/list pages
   - `index.html` - Homepage
2. Partials are in `layouts/partials/`
3. Assets are processed via Hugo pipes with fingerprinting and SRI

## Design System Summary

**Colors** (from pixel art header):
- Primary: `#3AAFB9` (teal)
- Secondary: `#59C265` (green)
- Accent: `#8B5D3B` (brown)

**Typography**:
- Headings: `Press Start 2P` (pixel font)
- Body: `Inter`

**Pixel Art Scaling** (targets 4 physical pixels per art pixel):
- 1x displays: `--art-pixel: 4px`
- 2x displays (Retina): `--art-pixel: 2px`
- 3x displays: `--art-pixel: 1.33px`

## Deployment

The site runs on a **Hetzner VPS (aarch64, NixOS)** behind **Caddy** as reverse proxy. The Rust server listens on localhost:8080, Caddy handles TLS.

```bash
mise run deploy         # Build, cross-compile, scp, restart
```

Infrastructure (NixOS config) is managed separately in `~/nixos-config/`:
- `modules/homepage.nix` - systemd service, user, Caddy virtualhost
- `hosts/palanthas/default.nix` - enables Caddy + homepage

See `server/README.md` for details.

## Key Files Reference

| File | Purpose |
|------|---------|
| `hugo.toml` | Site config, menu, social links, languages |
| `themes/wizard/layouts/_default/baseof.html` | Base HTML template |
| `themes/wizard/assets/css/style.css` | Main styles, theme variables |
| `themes/wizard/assets/js/light_dark.js` | Theme toggle logic |
| `themes/wizard/assets/js/memory-game.js` | Footer card game |
| `content-org/all-pages.org` | All page/post content |
| `server/build.rs` | Asset embedding for Rust server |
| `server/deploy-vps.sh` | VPS deployment script (scp + restart) |
| `flake.nix` | Nix dev shell with cross-compile toolchain |
| `.mise.toml` | Build, dev, and deploy tasks |
