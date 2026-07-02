#!/usr/bin/env bash
# Validate the generated HTML for correctness AND semantic structure, so the
# site stays readable with CSS disabled.
#
# Requires the nix dev shell (provides hugo, html5validator, node). The
# jsdom/axe-core deps for the semantic checker are installed on first run via
# `npm ci` (or `npm install` if there's no lockfile yet) into
# scripts/node_modules, mirroring how lighthouse pulls its tooling.
#
# Two layers, run over every page Hugo emits in public/:
#   1. W3C correctness  -> html5validator (wraps vnu.jar): nesting, bad/dupe
#      attributes, duplicate ids, missing alt, stray tags. Handles minified HTML.
#   2. Semantic + CSS-off + axe-core -> scripts/validate-html.mjs: one <main>,
#      one <h1>, landmarks, no skipped heading levels, links/buttons/images that
#      still make sense with the stylesheet off, plus axe accessibility rules.
#
# Exits non-zero if either layer reports an error on any page, so it doubles as
# a regression gate while editing templates.
#
# Usage:
#   scripts/validate-html.sh            # build + validate every page
#   scripts/validate-html.sh /de/about/ # validate a single already-built page
#   VALIDATE_SKIP_BUILD=1 scripts/validate-html.sh   # reuse existing public/
#   VALIDATE_STRICT=1 scripts/validate-html.sh       # treat warnings as errors

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PUBLIC="$REPO_ROOT/public"
SCRIPTS="$REPO_ROOT/scripts"
PATH_SUFFIX="${1:-}"

need() { command -v "$1" >/dev/null 2>&1 || { echo "Missing '$1'. Enter the nix dev shell (direnv allow / nix develop)." >&2; exit 1; }; }
need hugo
need node

# Layer 1 (W3C correctness) prefers a vendored, up-to-date vnu.jar run directly
# via `java -jar` — nixpkgs' html5validator bundles an ancient vnu (20.6.30)
# that rejects valid modern attributes like `fetchpriority`, and its --vnu-jar
# override is unreliable. Drop a current jar at scripts/vendor/vnu.jar (see
# README / CLAUDE.md) to use it; otherwise we fall back to html5validator.
VNU_JAR="$SCRIPTS/vendor/vnu.jar"
if [ -f "$VNU_JAR" ]; then
  need java
else
  need html5validator
fi

# --- Build (unless reusing an existing public/ or validating one page) --------
if [ -z "$PATH_SUFFIX" ] && [ "${VALIDATE_SKIP_BUILD:-}" != "1" ]; then
  if pgrep -f "hugo server" >/dev/null; then
    echo "ERROR: 'hugo server' is running; stop it first (pkill -f 'hugo server')." >&2
    exit 1
  fi
  echo "Building site (hugo --minify)..."
  rm -rf "$PUBLIC"
  ( cd "$REPO_ROOT" && hugo --minify >/dev/null )
fi

[ -d "$PUBLIC" ] || { echo "No public/ directory. Run without VALIDATE_SKIP_BUILD to build first." >&2; exit 1; }

# --- Ensure the semantic checker's deps are present --------------------------
if [ ! -d "$SCRIPTS/node_modules/jsdom" ] || [ ! -d "$SCRIPTS/node_modules/axe-core" ]; then
  echo "Installing semantic-checker deps (jsdom, axe-core)..."
  if [ -f "$SCRIPTS/package-lock.json" ]; then
    ( cd "$SCRIPTS" && npm ci --no-audit --no-fund )
  else
    ( cd "$SCRIPTS" && npm install --no-audit --no-fund )
  fi
fi

# --- Collect target files ----------------------------------------------------
declare -a FILES
if [ -n "$PATH_SUFFIX" ]; then
  # Accept "/de/about/", "de/about", or a direct file path.
  cand="$PUBLIC/${PATH_SUFFIX#/}"
  cand="${cand%/}"
  if [ -f "$cand" ]; then FILES=("$cand")
  elif [ -f "$cand/index.html" ]; then FILES=("$cand/index.html")
  else echo "No HTML found for '$PATH_SUFFIX' under public/." >&2; exit 1; fi
else
  while IFS= read -r f; do FILES+=("$f"); done < <(find "$PUBLIC" -name '*.html' | sort)
fi

echo "Validating ${#FILES[@]} page(s)..."
echo ""

fail=0

# --- Layer 1: W3C correctness -----------------------------------------------
if [ -f "$VNU_JAR" ]; then
  echo "=== [1/2] W3C correctness (vnu.jar, vendored) ==="
  vnu_cmd=(java -jar "$VNU_JAR" --format gnu)
else
  echo "=== [1/2] W3C correctness (html5validator) ==="
  vnu_cmd=(html5validator --format gnu)
fi
if "${vnu_cmd[@]}" "${FILES[@]}"; then
  echo "  all pages well-formed"
else
  echo "  ^ correctness errors above"
  fail=1
fi
echo ""

# --- Layer 2: semantic structure + CSS-off + axe ----------------------------
echo "=== [2/2] Semantic structure, CSS-off readability, axe-core ==="
if node "$SCRIPTS/validate-html.mjs" "${FILES[@]}"; then
  :
else
  fail=1
fi
echo ""

if [ "$fail" -ne 0 ]; then
  echo "VALIDATION FAILED."
  exit 1
fi
echo "VALIDATION PASSED."
