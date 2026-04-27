#!/usr/bin/env bash
set -euo pipefail

# Install rust & trunk if not present
if ! command -v trunk >/dev/null; then
  curl https://sh.rustup.rs -sSf | sh -s -- -y
  export PATH="$HOME/.cargo/bin:$PATH"
  rustup default stable
  cargo install trunk --locked
fi

export PATH="$HOME/.cargo/bin:$PATH"
# Build the site
trunk build --release
