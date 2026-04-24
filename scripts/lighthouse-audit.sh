#!/usr/bin/env bash
# Run Lighthouse (mobile + desktop) against the local Rust server and print a summary.
#
# Requires the nix dev shell (provides node, chromium, jq, hugo). Lighthouse
# itself is pulled from npm via npx on first run and cached thereafter.
#
# The Rust server (`mise run dev`) is the production-equivalent target: same
# brotli/gzip negotiation, same cache headers, same embedded assets as prod.
# Hugo dev server is NOT used — it lacks compression and realistic cache
# behavior.
#
# Usage:
#   scripts/lighthouse-audit.sh [url-path]
#
# Default target: http://localhost:8080/
# Output: .lighthouse/{mobile,desktop}.report.{html,json}

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="$REPO_ROOT/.lighthouse"
PATH_SUFFIX="${1:-/}"
PORT=8080
URL="http://localhost:$PORT$PATH_SUFFIX"

mkdir -p "$OUT_DIR"

if ! curl -fsS "http://localhost:$PORT/" -o /dev/null 2>&1; then
  echo "Nothing listening on :$PORT." >&2
  echo "Start the Rust server in another shell with: mise run dev" >&2
  exit 1
fi

CHROME_FLAGS="--headless=new --no-sandbox --disable-gpu --disable-dev-shm-usage"

summarize() {
  local json=$1
  local label=$2
  jq -r --arg label "$label" '
    def pct: . * 100 | floor;
    "--- " + $label + " scores ---",
    "  Performance:    " + (.categories.performance.score    | pct | tostring),
    "  Accessibility:  " + (.categories.accessibility.score  | pct | tostring),
    "  Best Practices: " + (.categories["best-practices"].score | pct | tostring),
    "  SEO:            " + (.categories.seo.score            | pct | tostring),
    "",
    "  Core Web Vitals:",
    "    LCP: " + (.audits["largest-contentful-paint"].displayValue // "n/a"),
    "    CLS: " + (.audits["cumulative-layout-shift"].displayValue // "n/a"),
    "    TBT: " + (.audits["total-blocking-time"].displayValue // "n/a"),
    "    FCP: " + (.audits["first-contentful-paint"].displayValue // "n/a"),
    "    SI:  " + (.audits["speed-index"].displayValue // "n/a"),
    "",
    "  Scored audits below 1.0:",
    ( [ .audits | to_entries[]
        | select(.value.score != null and .value.score < 1)
        | "    [" + (.value.score | tostring) + "] " + .key + " — " + (.value.title // "")
      ] | join("\n") )
  ' "$json"
}

run_lh() {
  local label=$1
  shift
  echo ""
  echo "=== Lighthouse $label ($URL) ==="
  npx -y -p lighthouse lighthouse "$URL" \
    --output=json --output=html \
    --output-path="$OUT_DIR/$label" \
    --chrome-flags="$CHROME_FLAGS" \
    --quiet \
    "$@"
  summarize "$OUT_DIR/$label.report.json" "$label"
}

# Mobile is the default form factor.
run_lh mobile
run_lh desktop --preset=desktop

echo ""
echo "Reports:"
echo "  $OUT_DIR/mobile.report.html"
echo "  $OUT_DIR/desktop.report.html"
