#!/usr/bin/env bash
# Generates THIRD_PARTY_LICENSES.txt: the third-party OSS licenses for the Rust
# crates STATICALLY LINKED into the shipped `adpt` binary. Because the release
# binary links its whole dependency closure, distributing it distributes that
# object code — so the dependencies' license terms must travel with the artifact.
#
# Output is regenerated per build and never committed (see .gitignore).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${1:-$ROOT/THIRD_PARTY_LICENSES.txt}"

log() { echo "[third-party-licenses] $*"; }

if ! command -v cargo-about >/dev/null 2>&1; then
    if [[ -x "$HOME/.cargo/bin/cargo-about" ]]; then
        export PATH="$HOME/.cargo/bin:$PATH"
    else
        echo "ERROR: cargo-about not found." >&2
        echo "Install it with: cargo install cargo-about --locked --features cli (or cargo binstall cargo-about)" >&2
        exit 1
    fi
fi
log "using cargo-about $(cargo-about --version)"

# `about.toml` carries the accepted-license allowlist and the shipped targets;
# `about.hbs` is the plain-text template. `[private] ignore = true` drops the
# first-party `adpt` crate so only third-party dependencies are emitted. --fail
# aborts if any crate's license is missing or outside the allowlist.
RAW="$(mktemp)"
trap 'rm -f "$RAW"' EXIT
log "generating licenses for the adpt dependency closure"
cargo about generate --locked --fail -c "$ROOT/about.toml" -o "$RAW" "$ROOT/about.hbs"
log "done ($(grep -c '^License:' "$RAW") license blocks)"

{
    cat <<EOF
================================================================================
adpt - Third-Party Software Notices
================================================================================
Generated at build time on $(date -u +"%Y-%m-%dT%H:%M:%SZ").

The adpt binary statically links the third-party crates listed below. Their
license terms are reproduced here, as required, because their object code is
distributed as part of this binary.

EOF
    cat "$RAW"
} > "$OUT"
log "wrote $OUT ($(du -h "$OUT" | cut -f1 | tr -d ' '))"
