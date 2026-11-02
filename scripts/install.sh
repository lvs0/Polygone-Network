#!/usr/bin/env bash
# Legacy URL wrapper. The repository-root install.sh is the sole installer.
# curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/lvs0/Polygone-Network/main/scripts/install.sh | bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" 2>/dev/null && pwd -P || true)"
if [[ -n "$SCRIPT_DIR" && -f "$SCRIPT_DIR/../install.sh" ]]; then
    exec bash "$SCRIPT_DIR/../install.sh" "$@"
fi

TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/polygone-installer.XXXXXX")"
trap 'rm -rf -- "$TMP_DIR"' EXIT INT TERM
INSTALLER_URL="${POLYGONE_INSTALLER_URL:-https://raw.githubusercontent.com/lvs0/Polygone-Network/main/install.sh}"

curl --proto '=https' --tlsv1.2 --proto-redir '=https' -fsSL "$INSTALLER_URL" -o "$TMP_DIR/install.sh"
bash "$TMP_DIR/install.sh" "$@"
