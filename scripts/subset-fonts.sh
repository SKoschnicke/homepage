#!/usr/bin/env bash
# Regenerate the served Monaspace Argon font from the pristine upstream file.
#
# Upstream ships three variable axes and wide glyph coverage; the site uses
# only weights 400-700, never width/slant, and Latin text. So:
#   1. pin wdth=100 and slnt=0, clamp wght to 400:700 (varLib.instancer)
#   2. keep a fixed set of Unicode ranges generous enough for future posts
#      (NOT derived from current content - new chars must not silently fall back)
#   3. keep ALL layout features: texture healing lives in `calt`
#
# Needs fonttools + brotli (nix dev shell). Run after upgrading the source font.
set -euo pipefail
cd "$(dirname "$0")/.."

SRC=themes/wizard/fonts-src/monaspace-argon-variable.woff2
OUT=themes/wizard/static/fonts/monaspace-argon-variable.woff2

UNICODES=(
    U+0020-007E   # Basic Latin
    U+00A0-00FF   # Latin-1 Supplement (German, French, Spanish, § © · ×)
    U+0100-017F   # Latin Extended-A (European names)
    U+0300-036F   # Combining Diacritical Marks (keeps ccmp working)
    U+0370-03FF   # Greek (λ, π, ...; note: upstream has no μ)
    U+2010-205E   # General Punctuation (dashes, curly quotes, …, bullets)
    U+20AC        # Euro sign
    U+2190-21FF   # Arrows
    U+2200-22FF   # Mathematical Operators (≠ ≤ ≥ −)
    U+2500-257F   # Box Drawing (tree diagrams in code blocks)
)

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

python3 -m fontTools.varLib.instancer "$SRC" wdth=100 slnt=0 wght=400:700 \
    --quiet -o "$tmp/instanced.ttf"

pyftsubset "$tmp/instanced.ttf" \
    --unicodes="$(IFS=,; echo "${UNICODES[*]}")" \
    --layout-features='*' \
    --flavor=woff2 \
    --output-file="$OUT"

printf '%s: %s -> %s bytes\n' "$OUT" "$(stat -c %s "$SRC")" "$(stat -c %s "$OUT")"
