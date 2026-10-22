#!/usr/bin/env bash
# Vercel build script for PdfCraft Web (WASM)
set -euo pipefail

export PATH="$HOME/.cargo/bin:$PATH"

echo "=== PdfCraft Web Build for Vercel ==="

# 1. Ensure Rust toolchain and wasm32 target exist
if ! command -v rustup >/dev/null 2>&1; then
  echo "--> Installing Rust toolchain (minimal profile)..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --target wasm32-unknown-unknown --profile minimal
  # shellcheck source=/dev/null
  source "$HOME/.cargo/env"
else
  echo "--> Checking wasm32-unknown-unknown target..."
  rustup target add wasm32-unknown-unknown || true
fi

# 2. Ensure trunk is installed (download precompiled binary for fast CI build)
if ! command -v trunk >/dev/null 2>&1; then
  echo "--> Fetching precompiled Trunk binary for Linux x86_64..."
  mkdir -p "$HOME/.cargo/bin"
  TRUNK_VERSION="v0.22.0"
  curl -fsSL "https://github.com/trunk-rs/trunk/releases/download/${TRUNK_VERSION}/trunk-x86_64-unknown-linux-gnu.tar.gz" | tar -xz -C "$HOME/.cargo/bin"
  chmod +x "$HOME/.cargo/bin/trunk"
fi

echo "--> Rustc version: $(rustc --version)"
echo "--> Trunk version: $(trunk --version)"

# 3. Build WebAssembly release
echo "--> Building PdfCraft WebAssembly site with Trunk..."
ROOT="$(pwd)"
cd "$ROOT/apps/pdfcraft-web"
trunk build --release

# 4. Verify output
cd "$ROOT"
if [ ! -f "dist/web/index.html" ]; then
  echo "error: dist/web/index.html was not created!" >&2
  exit 1
fi

echo "=== Build Complete! Static output located in dist/web ==="
ls -lh dist/web
