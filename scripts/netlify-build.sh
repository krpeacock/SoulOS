#!/usr/bin/env bash
set -euo pipefail

# Netlify's /opt/build/cache persists between builds on the same site.
# Redirect Cargo/Rustup/Trunk there so the install only happens once —
# every subsequent build finds them already cached.
export CARGO_HOME=/opt/build/cache/cargo
export RUSTUP_HOME=/opt/build/cache/rustup
# Trunk downloads wasm-bindgen-cli and wasm-opt under XDG_CACHE_HOME.
export XDG_CACHE_HOME=/opt/build/cache/xdg
export PATH="$CARGO_HOME/bin:$PATH"

if ! command -v rustup >/dev/null 2>&1; then
  curl https://sh.rustup.rs -sSf | sh -s -- -y --no-modify-path \
    --default-toolchain stable
fi

rustup default stable
rustup target add wasm32-unknown-unknown

# Force trunk to a version that supports `data-wasm-opt-params` on
# the `<link rel="rust">` tag in index.html. Older trunk silently
# ignored that attribute, which is how Netlify started failing with
# `Bulk memory operations require bulk memory [--enable-bulk-memory]`
# despite the index.html clearly passing the flag — Netlify's persistent
# `/opt/build/cache` was holding onto a pre-0.21 trunk binary. The
# `--force` reinstall is the cheapest reliable fix; cargo's incremental
# build still keeps it fast.
TRUNK_VERSION=0.21.14
if ! command -v trunk >/dev/null 2>&1 || \
   [ "$(trunk --version 2>/dev/null | awk '{print $2}')" != "$TRUNK_VERSION" ]; then
  cargo install trunk --locked --version "$TRUNK_VERSION" --force
fi

trunk build --release
