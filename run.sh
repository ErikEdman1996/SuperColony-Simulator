#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"

# Prefer an ordinary Rust installation. The fallback is the isolated toolchain
# installed for this prototype in the current OpenCode/WSL environment.
if ! command -v cargo >/dev/null 2>&1; then
    if [[ -x /tmp/opencode/supercolony-cargo/bin/cargo ]]; then
        export CARGO_HOME=/tmp/opencode/supercolony-cargo
        export RUSTUP_HOME=/tmp/opencode/supercolony-rustup
        export PATH="$CARGO_HOME/bin:$PATH"
        export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/opencode/supercolony-target}"
    else
        echo 'Rust is required. Install it from https://rustup.rs, then rerun this script.' >&2
        exit 1
    fi
fi
exec cargo run --release --bin supercolony -- "$@"
