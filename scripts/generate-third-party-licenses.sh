#!/usr/bin/env bash
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
