#!/usr/bin/env bash
# Local build script for PdfCraft Web (macOS / Linux)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export PATH="$HOME/.cargo/bin:$PATH"

echo "=== Building PdfCraft Web locally ==="

# 1. Check Rust
if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo is not installed. Please install Rust via https://rustup.rs" >&2
  exit 1
fi

# 2. Check wasm32 target
if command -v rustup >/dev/null 2>&1; then
  rustup target add wasm32-unknown-unknown 2>/dev/null || true
fi

# 3. Check Trunk
if ! command -v trunk >/dev/null 2>&1; then
  OS="$(uname -s)"
  ARCH="$(uname -m)"
  TRUNK_VERSION="v0.22.0"
  echo "Trunk not found in PATH. Attempting to download precompiled binary for $OS $ARCH..."
  
  DOWNLOAD_URL=""
  if [ "$OS" = "Darwin" ]; then
    if [ "$ARCH" = "arm64" ]; then
      DOWNLOAD_URL="https://github.com/trunk-rs/trunk/releases/download/${TRUNK_VERSION}/trunk-aarch64-apple-darwin.tar.gz"
    else
      DOWNLOAD_URL="https://github.com/trunk-rs/trunk/releases/download/${TRUNK_VERSION}/trunk-x86_64-apple-darwin.tar.gz"
    fi
  elif [ "$OS" = "Linux" ]; then
    DOWNLOAD_URL="https://github.com/trunk-rs/trunk/releases/download/${TRUNK_VERSION}/trunk-x86_64-unknown-linux-gnu.tar.gz"
  fi

  if [ -n "$DOWNLOAD_URL" ]; then
    mkdir -p "$HOME/.cargo/bin"
    curl -fsSL "$DOWNLOAD_URL" | tar -xz -C "$HOME/.cargo/bin"
    chmod +x "$HOME/.cargo/bin/trunk"
    echo "Trunk installed to $HOME/.cargo/bin/trunk"
  else
    echo "Could not auto-download trunk. Please run: cargo install trunk --locked" >&2
    exit 1
  fi
fi

# 4. Build
cd "$ROOT/apps/pdfcraft-web"
echo "Running trunk build --release..."
trunk build --release

echo ""
echo "=== Build finished! ==="
echo "The static web application is in: $ROOT/dist/web"
echo "To preview locally, run: python3 -m http.server 8765 --directory $ROOT/dist/web"
