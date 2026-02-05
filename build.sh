#!/bin/bash
set -e

# Note: Gemini content is exported separately via ox-gemini in Emacs
# Run your org-mode export before this script if you've updated content
# The gemini-content/ directory should contain .gmi files

echo "Building static site with Hugo..."
hugo --minify

echo "Building Rust server..."
cd server
cargo build --release

echo "Stripping debug symbols..."
strip target/release/static-server

echo ""
echo "Build complete!"
echo "Binary: server/target/release/static-server"
echo "Size: $(du -h target/release/static-server | cut -f1)"
echo ""
echo "To run locally:"
echo "  cd server && cargo run --release"
echo ""
echo "To build unikernel:"
echo "  cd server && ops build target/release/static-server -c config.json"
