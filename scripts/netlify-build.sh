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

rustup target add wasm32-unknown-unknown

if ! command -v trunk >/dev/null 2>&1; then
  cargo install trunk --locked
fi

trunk build --release
