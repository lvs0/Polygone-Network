#!/usr/bin/env bash
# Smoke commands for the currently supported Polygone CLI.
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

TMP_HOME="$(mktemp -d)"
trap 'rm -rf "$TMP_HOME"' EXIT

smoke() {
    printf '+ polygone'
    printf ' %q' "$@"
    printf '\n'
    HOME="$TMP_HOME" cargo run --quiet --bin polygone -- "$@" >/dev/null
}

smoke --help
smoke clef
smoke id
smoke envoyer --help
smoke recevoir --help
smoke ecouter --help
smoke petals --help

echo "Smoke commands completed"
