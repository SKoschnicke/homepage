# CSS Architecture & Design System

This document describes the CSS architecture, design tokens, and styling patterns used in the wizard theme.

## File Organization

```
themes/wizard/assets/css/
├── style.css            # Main styles, CSS variables, typography, layout
├── header.css           # Header image, navigation, theme toggle
├── memory-game.css      # Footer card game styles
└── metrics-dashboard.css # Server metrics display
```

## Design Tokens (CSS Variables)

All colors and styling values are defined as CSS custom properties in `:root`.

### Color Palette

Derived from the pixel art header image:

| Variable | Light | Dark | Source |
|----------|-------|------|--------|
| `--primary-color` | `#3AAFB9` | `#3AAFB9` | Teal sky |
| `--secondary-color` | `#59C265` | `#59C265` | Green tree |
| `--accent-color` | `#8B5D3B` | `#8B5D3B` | Brown trunk |
| `--background-color` | `#F5F5E6` | `#1A2A33` | Cream / Dark blue-gray |
| `--text-color` | `#2A3B47` | `#E0E6EB` | Dark / Light text |
| `--link-color` | `#3AAFB9` | `#3AAFB9` | Teal |
| `--link-hover-color` | `#59C265` | `#59C265` | Green |

### Typography

```css
:root {
    --heading-font: 'Press Start 2P', monospace;
    --body-font: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI',
                 Roboto, Oxygen, Ubuntu, Cantarell, 'Open Sans',
                 'Helvetica Neue', sans-serif;
}
```

| Element | Font | Size |
|---------|------|------|
| Headings | Press Start 2P | h1: 1.8rem, h2: 1.5rem, h3: 1.2rem |
| Body text | Inter | 16px base |
| Navigation | Press Start 2P | 0.8rem |
| Code | Courier New | inherited |

### Layout

```css
:root {
    --max-width: 800px;
    --pixel-border: 2px solid var(--border-color);
    --art-pixel: 2px;  /* Desktop: 2x scale */
}
```

## Theme Switching

Themes are controlled via `data-theme` attribute on `<html>`:

```css
/* Light theme (default) */
:root {
    --background-color: #F5F5E6;
    --text-color: #2A3B47;
    /* ... */
}

/* Dark theme */
[data-theme="dark"] {
    --background-color: #1A2A33;
    --text-color: #E0E6EB;
    /* ... */
}
```

JavaScript toggles this attribute and persists to localStorage.

## Pixel Art Scaling

All pixel art images are stored at **1x native resolution** (1 art pixel = 1 file pixel) and scaled up via CSS. The goal is **2 physical pixels per art pixel** regardless of display density.

### Image Dimensions

| Image | Art pixels | File |
|-------|-----------|------|
| Header | 766×510 | `/static/images/header-tower.png` |
| Homepage | 98×206 | `/static/ox-hugo/*.png` |

### Optimizing Pixel Art Images

Use `ppa` (pixel-perfect-art) to convert images to indexed PNG with a limited color palette:

```bash
ppa -c 256 -w 4 -o output.png input.png
```

Options:
- `-c 256` - Limit to 256 colors (indexed palette)
- `-w 4` - Window size for color quantization
- `-o output.png` - Output file

This significantly reduces file size while preserving pixel art quality.

### CSS Implementation

```css
.header-image {
    /* Header art is 766x510 pixels */
    height: calc(510 * var(--art-pixel));
    background-size: calc(766 * var(--art-pixel)) calc(510 * var(--art-pixel));

    /* Preserve crisp pixels when scaling */
    image-rendering: pixelated;
    image-rendering: -moz-crisp-edges;
    image-rendering: crisp-edges;
}

.homepage-image img {
    /* Homepage art is 98x206 pixels */
    width: calc(98 * var(--art-pixel));
    height: calc(206 * var(--art-pixel));
    image-rendering: pixelated;
}
```

### DPR-Based Scaling (Progressive Enhancement)

The system uses **CSS media queries as fallback** with **JavaScript for precise values**:

**CSS fallback** (works without JS):
```css
:root { --art-pixel: 2px; }              /* 1x displays */

@media (min-resolution: 1.5dppx) {
    :root { --art-pixel: 1.33px; }       /* 2 / 1.5 */
}
@media (min-resolution: 2dppx) {
    :root { --art-pixel: 1px; }          /* 2 / 2 (Retina) */
}
@media (min-resolution: 2.5dppx) {
    :root { --art-pixel: 0.8px; }        /* 2 / 2.5 */
}
@media (min-resolution: 3dppx) {
    :root { --art-pixel: 0.67px; }       /* 2 / 3 */
}
```

**JavaScript fine-tuning** (`pixel-scale.js`):
```javascript
const PHYSICAL_PIXELS_TARGET = 2;
const artPixel = PHYSICAL_PIXELS_TARGET / window.devicePixelRatio;
document.documentElement.style.setProperty('--art-pixel', artPixel + 'px');
```

The JS calculates exact values for any DPR (not just the fixed breakpoints) and updates if the window moves between displays.

| Display DPR | CSS fallback | JS precise | Physical pixels |
|-------------|--------------|------------|-----------------|
| 1x | 2px | 2px | 2 |
| 1.5x | 1.33px | 1.33px | 2 |
| 2x (Retina) | 1px | 1px | 2 |
| 2.5x | 0.8px | 0.8px | 2 |
| 3x | 0.67px | 0.67px | 2 |

## Component Patterns

### Container

```css
.container {
    max-width: var(--max-width);
    margin: 0 auto;
    border: var(--pixel-border);
    background: var(--container-bg);
    backdrop-filter: blur(5px);
}
```

### Links

```css
a {
    color: var(--link-color);
    text-decoration: none;
    transition: all 0.3s ease;
}

a:hover {
    color: var(--link-hover-color);
    text-shadow: 0 0 8px var(--link-color);
}

/* Visited link indicator */
a:visited::after {
    content: '★';
    font-size: 0.8em;
    margin-left: 0.3em;
    animation: star-pulse 2s infinite;
}
```

### Post Cards

```css
.post-summary {
    padding: 1.5rem;
    border: var(--pixel-border);
    background: var(--container-bg);
    transition: all 0.3s ease;
}

.post-summary:hover {
    transform: translateY(-2px);
    box-shadow: 0 0 20px var(--primary-color);
}
```

### Buttons

```css
.btn-light-dark {
    background-color: transparent;
    border: var(--pixel-border);
    padding: 0.5rem 1rem;
    font-family: var(--heading-font);
    cursor: pointer;
    transition: all 0.3s ease;
}

.btn-light-dark:hover {
    background-color: var(--primary-color);
    color: var(--background-color);
    transform: scale(1.05);
}
```

### Code Blocks

```css
pre, code {
    background-color: var(--code-bg);
    font-family: 'Courier New', monospace;
}

pre {
    padding: 1rem;
    border: var(--pixel-border);
    overflow: scroll;
}
```

## Responsive Breakpoints

Layout breakpoints (pixel scaling is handled separately via DPR, see above):

```css
@media (max-width: 48em) {
    .header-content {
        padding: 0.5rem;
    }

    .btn-light-dark {
        width: 2.5rem;
        height: 2.5rem;
    }
}

@media (max-width: 600px) {
    .homepage-image {
        float: none;       /* Stack instead of float */
        max-width: 100%;
        margin: 0 auto 1.5rem auto;
    }
}
```

## Animation Patterns

### Star Pulse (visited links)

```css
@keyframes star-pulse {
    0% { transform: scale(1); opacity: 0.7; }
    50% { transform: scale(1.2); opacity: 1; }
    100% { transform: scale(1); opacity: 0.7; }
}
```

### Neon Glow (hover effects)

```css
:root {
    --neon-glow: 0 0 5px rgba(58, 175, 185, 0.5),
                 0 0 10px rgba(58, 175, 185, 0.3),
                 0 0 15px rgba(58, 175, 185, 0.1);
}
```

## Self-Hosted Fonts

All fonts are self-hosted with multiple unicode-range variants:

```css
@font-face {
    font-family: 'Press Start 2P';
    font-style: normal;
    font-weight: 400;
    font-display: swap;
    src: url('/fonts/press-start-2p-latin.woff2') format('woff2');
    unicode-range: U+0000-00FF, U+0131, ...;
}

@font-face {
    font-family: 'Inter';
    font-style: normal;
    font-weight: 400 700;
    font-display: swap;
    src: url('/fonts/inter-400-latin.woff2') format('woff2');
    unicode-range: U+0000-00FF, U+0131, ...;
}
```

Font files are in `/public/fonts/` after Hugo build.

## Common Modifications

### Adding a New Color

1. Add to `:root` in `style.css`:
   ```css
   :root {
       --new-color: #FF6B6B;
   }
   ```

2. Add dark theme variant in `[data-theme="dark"]`:
   ```css
   [data-theme="dark"] {
       --new-color: #FF8E8E;
   }
   ```

### Changing Typography

1. Modify font variables in `:root`:
   ```css
   :root {
       --heading-font: 'Your Font', monospace;
   }
   ```

2. Add `@font-face` if using custom font

### Adding Responsive Styles

Use the existing breakpoint or add new ones:

```css
/* Tablet */
@media (max-width: 48em) {
    .your-selector {
        /* tablet styles */
    }
}

/* Mobile */
@media (max-width: 600px) {
    .your-selector {
        /* mobile styles */
    }
}
```

## Testing Checklist

When modifying CSS:

1. [ ] Test light theme
2. [ ] Test dark theme
3. [ ] Test on 1x display (or simulate in DevTools)
4. [ ] Test on 2x Retina display
5. [ ] Test narrow viewport (<600px) for layout changes
6. [ ] Check pixel art renders crisply (no blurring)
7. [ ] Verify link hover states
8. [ ] Check button hover/active states
9. [ ] Validate code block readability
