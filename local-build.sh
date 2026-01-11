#!/bin/bash

set -e  # Exit on any error

# Detect platform (similar logic to cli.js)
PLATFORM="$(uname -s)_$(uname -m)"
case "$PLATFORM" in
  Linux_x86_64) PLATFORM="linux-x64" ;;
  Linux_aarch64) PLATFORM="linux-arm64" ;;
  Darwin_x86_64) PLATFORM="macos-x64" ;;
  Darwin_arm64) PLATFORM="macos-arm64" ;;
  MINGW*_x86_64|MSYS*_x86_64) PLATFORM="windows-x64" ;;
  MINGW*_aarch64|MSYS*_aarch64) PLATFORM="windows-arm64" ;;
  *)
    echo "Unsupported platform: $PLATFORM"
    exit 1
    ;;
esac

echo "🧹 Cleaning previous builds..."
rm -rf npx-cli/dist
mkdir -p "npx-cli/dist/$PLATFORM"

echo "🔨 Building frontend..."
(cd frontend && npm run build)

echo "🔨 Building Rust binaries..."
cargo build --release --manifest-path Cargo.toml
cargo build --release --bin mcp_task_server --manifest-path Cargo.toml

echo "📦 Creating distribution package..."

# Copy the main binary
cp target/release/server vibe-kanban
zip -q vibe-kanban.zip vibe-kanban
rm -f vibe-kanban
mv vibe-kanban.zip "npx-cli/dist/$PLATFORM/vibe-kanban.zip"

# Copy the MCP binary
cp target/release/mcp_task_server vibe-kanban-mcp
zip -q vibe-kanban-mcp.zip vibe-kanban-mcp
rm -f vibe-kanban-mcp
mv vibe-kanban-mcp.zip "npx-cli/dist/$PLATFORM/vibe-kanban-mcp.zip"

# Copy the Review CLI binary
cp target/release/review vibe-kanban-review
zip -q vibe-kanban-review.zip vibe-kanban-review
rm -f vibe-kanban-review
mv vibe-kanban-review.zip "npx-cli/dist/$PLATFORM/vibe-kanban-review.zip"

echo "✅ Build complete!"
echo "📁 Files created:"
echo "   - npx-cli/dist/$PLATFORM/vibe-kanban.zip"
echo "   - npx-cli/dist/$PLATFORM/vibe-kanban-mcp.zip"
echo "   - npx-cli/dist/$PLATFORM/vibe-kanban-review.zip"
echo ""
echo "🚀 To test locally, run:"
echo "   cd npx-cli && node bin/cli.js"
