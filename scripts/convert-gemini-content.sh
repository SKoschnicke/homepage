#!/usr/bin/env bash
# Convert Gemini content body using Pandoc
#
# This script post-processes Hugo's .gmi output:
# 1. Finds .gmi files with <!-- GEMINI_BODY --> placeholder
# 2. Locates corresponding source markdown
# 3. Converts body through Pandoc with custom Lua writer
# 4. Replaces placeholder with converted content

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
PUBLIC_DIR="$PROJECT_ROOT/public"
CONTENT_DIR="$PROJECT_ROOT/content"
LUA_WRITER="$SCRIPT_DIR/gemini-writer.lua"

PLACEHOLDER="<!-- GEMINI_BODY -->"

# Check dependencies
if ! command -v pandoc &> /dev/null; then
    echo "Error: pandoc is required but not installed"
    exit 1
fi

if [ ! -f "$LUA_WRITER" ]; then
    echo "Error: Lua writer not found at $LUA_WRITER"
    exit 1
fi

# Extract markdown body (strip TOML front matter)
extract_body() {
    local file="$1"
    # TOML front matter: +++ ... +++
    # Skip lines until we see closing +++, then output the rest
    awk '
        BEGIN { in_frontmatter = 0; seen_opener = 0 }
        /^\+\+\+/ {
            if (!seen_opener) {
                seen_opener = 1
                in_frontmatter = 1
                next
            } else if (in_frontmatter) {
                in_frontmatter = 0
                next
            }
        }
        !in_frontmatter && seen_opener { print }
    ' "$file"
}

# Convert a single .gmi file
convert_file() {
    local gmi_file="$1"

    # Check if file contains placeholder
    if ! grep -q "$PLACEHOLDER" "$gmi_file" 2>/dev/null; then
        return 0
    fi

    # Determine source markdown path
    # public/about/index.gmi -> content/about.md
    # public/posts/hello-world/index.gmi -> content/posts/hello-world.md
    local rel_path="${gmi_file#$PUBLIC_DIR/}"
    rel_path="${rel_path%/index.gmi}"

    local md_file="$CONTENT_DIR/${rel_path}.md"

    if [ ! -f "$md_file" ]; then
        echo "Warning: Source not found for $gmi_file (expected $md_file)"
        return 0
    fi

    echo "  Converting: $rel_path"

    # Extract body and convert with Pandoc
    local converted
    converted=$(extract_body "$md_file" | pandoc -f markdown -t "$LUA_WRITER")

    # Replace placeholder with converted content
    # Use a temp file for safe replacement
    local temp_file
    temp_file=$(mktemp)

    awk -v placeholder="$PLACEHOLDER" -v content="$converted" '
        {
            if (index($0, placeholder)) {
                # Replace the placeholder line with converted content
                print content
            } else {
                print
            }
        }
    ' "$gmi_file" > "$temp_file"

    mv "$temp_file" "$gmi_file"
}

# Main
echo "Converting Gemini content with Pandoc..."

# Find all .gmi files and process them
find "$PUBLIC_DIR" -name "index.gmi" -type f | while read -r gmi_file; do
    convert_file "$gmi_file"
done

echo "Gemini conversion complete."
