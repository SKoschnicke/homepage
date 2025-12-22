# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Personal homepage built with Hugo, featuring a custom pixel art "Wizard" theme inspired by fantasy and retro gaming aesthetics. Content is authored in Emacs Org-mode and exported to Markdown.

## Content Workflow

**IMPORTANT: Content is authored in `content-org/all-pages.org`, not directly in Markdown files.**

- All page content lives in `/content-org/all-pages.org` as Emacs Org-mode headings
- Each heading has PROPERTIES including `EXPORT_FILE_NAME` and optionally an `ID`
- Org-mode exports these to Markdown files in `/content/` directory
- **DO NOT edit Markdown files in `/content/` directly** - they are auto-generated from the org file
- To modify content: edit the corresponding heading in `all-pages.org`

Example org structure:
```org
** DONE Magic Wormhole                                                         :software:
:PROPERTIES:
:EXPORT_FILE_NAME: magic-wormhole
:ID:       d25b0e33-8bd4-44cc-9249-dcfd4eff5b1a
:END:

[Content goes here]
```

## Hugo Commands

Hugo is installed via Nix. Current version: 0.136.5+extended

```bash
# Development server with live reload
hugo server -D

# Build the site (output to /public/)
hugo

# Build for production with minification
hugo --minify
```

## Theme Architecture

The site uses a custom "wizard" theme located in `themes/wizard/`. This theme:

- **Overlays the "poison" base theme** (`themes/wizard/poison/`) - only override files that need customization
- Uses a dual-theme approach: wizard-specific layouts/assets override poison defaults
- **Directly modify theme files** - this is a site-specific theme, not a reusable component

Key theme files:
- `themes/wizard/layouts/_default/baseof.html` - Base template with header image, navigation, footer
- `themes/wizard/assets/css/style.css` - Main stylesheet with light/dark theme CSS variables
- `themes/wizard/assets/css/header.css` - Header image styling
- `themes/wizard/assets/css/memory-game.css` - Memory card game in footer
- `themes/wizard/assets/js/light_dark.js` - Theme toggle functionality
- `themes/wizard/assets/js/memory-game.js` - Interactive memory game

## Design System

Color palette (from pixel art header):
- Primary: `#3AAFB9` (teal sky)
- Secondary: `#59C265` (green tree)
- Accent: `#8B5D3B` (brown trunk)

Typography:
- Headings: `Press Start 2P` (pixel/retro aesthetic)
- Body: `Inter` (modern readability)

Both light and dark themes use the same color scheme with inverted backgrounds.

## Design Principles (from Cursor rules)

- Write minimal semantic HTML following modern best practices
- Structure CSS for small file size; split into multiple files if needed
- **Always consider both light and dark modes** when making changes
- **Always ensure mobile AND desktop responsiveness** - use additional desktop space effectively
- When changing design, modify theme files directly (not meant to be reusable)
- Keep README.md updated with structure, design decisions, and TODO list

## Project Structure

```
content-org/all-pages.org  # Source of truth for all content
content/                   # Generated markdown (do not edit)
themes/wizard/             # Custom theme (modify directly)
  poison/                  # Base theme (rarely modified)
  layouts/                 # Hugo templates
  assets/                  # CSS, JS, images
static/images/             # Static assets
hugo.toml                  # Site configuration
server/                    # Rust unikernel web server
  README.md                # Server documentation
  Cargo.toml               # Rust dependencies
  build.rs                 # Asset embedding (build-time)
  src/                     # Server source code
build.sh                   # Integration script (Hugo + Rust)
```

## Deployment

### GitHub Pages (Current)

Site deploys automatically to GitHub Pages via `.github/workflows/hugo.yml` on push to main branch. The workflow:
- Uses Hugo v0.140.2 (extended)
- Builds with `--minify` flag
- Deploys to `https://sven.guru/`

The `/public` directory is git-ignored locally but built in CI. A symlink `docs -> public` exists for local preview.

### Rust Unikernel Server (Alternative)

A high-performance Rust web server is available in `/server/` for unikernel deployment. This server:
- Embeds the entire Hugo site into a single binary (~7.6MB)
- Pre-compresses all assets at build time (zero runtime compression overhead)
- Serves from static memory with zero-copy I/O
- Targets <100μs latency vs nginx's ~200-500μs

**See `/server/README.md` for complete documentation.**

Quick start:
```bash
# Build site and server
./build.sh

# Run locally
cd server && cargo run --release

# Build unikernel (requires ops)
cd server
ops build target/release/static-server -c config.json
```

The server uses:
- `hyper` for HTTP (direct, no framework overhead)
- Build-time asset embedding via `build.rs`
- Pre-compressed variants (gzip + brotli) for all text assets
- Aggressive caching headers for Hugo's fingerprinted assets

When updating Hugo content, rebuild the Rust server to embed new assets.
