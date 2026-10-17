#!/usr/bin/env bash
# Minimal role-based smoke test for an actual Polygone relay transfer.
# This script prepares the test; it does not claim a two-host run by itself.
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
POLYGONE_BIN="${POLYGONE_BIN:-$REPO_ROOT/target/release/polygone}"
POLYGONE_RELAY_BIN="${POLYGONE_RELAY_BIN:-$REPO_ROOT/target/release/polygone-relay}"

usage() {
    cat <<'USAGE'
Usage:
  scripts/smoke-two-hosts.sh relay [port]
  scripts/smoke-two-hosts.sh listen <relay-host:port>
  POLYGONE_SEND_TIMEOUT_SECONDS=300 scripts/smoke-two-hosts.sh send <relay-host:port> <destination-node-id> <destination-ML-KEM-public-key-hex>

Set POLYGONE_BIN and/or POLYGONE_RELAY_BIN to use binaries outside target/release.
For send, POLYGONE_SEND_TIMEOUT_SECONDS configures the timeout (default: 180 s;
allowed range: 1-3600 s). GNU coreutils 'timeout' is required.
The send role creates a unique 3 x 12-KiB synthetic file, sends it, then asks
for the receiver's BLAKE3 and compares it with the sender's BLAKE3.
USAGE
}

require_binary() {
    local binary="$1"
    local label="$2"
    if [[ ! -x "$binary" ]]; then
        printf 'Erreur : binaire %s absent/non exécutable : %s\n' "$label" "$binary" >&2
        printf 'Construire avec : cargo build --release --bin polygone --bin polygone-relay\n' >&2
        exit 2
    fi
}

role="${1:-}"
if [[ -z "$role" || "$role" == "-h" || "$role" == "--help" ]]; then
    usage
    exit 0
fi
shift

case "$role" in
    relay)
        if (( $# > 1 )); then usage >&2; exit 2; fi
        port="${1:-7000}"
        if [[ ! "$port" =~ ^[0-9]{1,5}$ ]] || (( 10#$port < 1 || 10#$port > 65535 )); then
            printf 'Erreur : port TCP invalide : %s\n' "$port" >&2
            exit 2
        fi
        require_binary "$POLYGONE_RELAY_BIN" "polygone-relay"
        printf 'Relay TCP sur le port %s (Ctrl-C pour arrêter).\n' "$port"
        exec "$POLYGONE_RELAY_BIN" --port "$port"
        ;;
    listen)
        if (( $# != 1 )); then usage >&2; exit 2; fi
        require_binary "$POLYGONE_BIN" "polygone"
        relay_addr="$1"
        peers_store="${HOME:-.}/.polygone/peers.json"
        peers_marker="${peers_store}.initialized"
        if [[ ! -e "$peers_store" && ! -L "$peers_store" && ! -e "$peers_marker" && ! -L "$peers_marker" ]]; then
            printf 'Premier démarrage : initialisation explicite du store de confiance.\n'
            "$POLYGONE_BIN" initialiser-pairs
        fi
        printf 'Écoute via %s; laisser cette commande active (Ctrl-C pour arrêter).\n' "$relay_addr"
        # Keep the listener as this process so its stdout/stderr and exit code
        # remain available to the caller's process handle.
        exec "$POLYGONE_BIN" ecouter --relay "$relay_addr"
        ;;
    send)
        if (( $# != 3 )); then usage >&2; exit 2; fi
        relay_addr="$1"
        destination_node="$2"
        destination_kem_pk="$3"
        if [[ ! "$destination_node" =~ ^[[:xdigit:]]{16}$ ]]; then
            printf 'Erreur : node_id attendu sous forme de 16 caractères hexadécimaux.\n' >&2
            exit 2
        fi
        if [[ ! "$destination_kem_pk" =~ ^[[:xdigit:]]+$ ]]; then
            printf 'Erreur : la clé publique ML-KEM doit être une chaîne hexadécimale.\n' >&2
            exit 2
        fi
        require_binary "$POLYGONE_BIN" "polygone"
        if ! command -v timeout >/dev/null 2>&1; then
            printf "Erreur : GNU coreutils 'timeout' est requis pour le rôle send.\n" >&2
            exit 2
        fi
        timeout_version="$(timeout --version 2>/dev/null || true)"
        if [[ "$timeout_version" != *coreutils* ]]; then
            printf "Erreur : GNU coreutils 'timeout' est requis pour le rôle send.\n" >&2
            exit 2
        fi

        send_timeout_seconds="${POLYGONE_SEND_TIMEOUT_SECONDS:-180}"
        if [[ ! "$send_timeout_seconds" =~ ^[0-9]{1,4}$ ]] || (( 10#$send_timeout_seconds < 1 || 10#$send_timeout_seconds > 3600 )); then
            printf 'Erreur : POLYGONE_SEND_TIMEOUT_SECONDS doit être un entier entre 1 et 3600 (secondes).\n' >&2
            exit 2
        fi

        tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/polygone-two-host-smoke.XXXXXX")"
        trap 'rm -rf -- "$tmp_dir"' EXIT
        token="$(basename -- "$tmp_dir" | tr -cd '[:alnum:]')"
        payload="$tmp_dir/polygone-smoke-${token}.bin"
        drive_block_bytes=12288
        synthetic_blocks=3
        : > "$payload"
        for ((block = 1; block <= synthetic_blocks; block++)); do
            marker="Polygone two-host smoke block ${block}/${synthetic_blocks}; nonce=${token}"$'\n'
            marker_bytes="${#marker}"
            padding=$((drive_block_bytes - marker_bytes))
            printf '%s' "$marker" >> "$payload"
            printf '%*s' "$padding" '' | tr ' ' 'x' >> "$payload"
        done

        payload_bytes="$(wc -c < "$payload" | tr -d '[:space:]')"
        printf 'Fichier-témoin : %s (%s octets ; %s blocs Drive de 12 Kio)\n' "$(basename -- "$payload")" "$payload_bytes" "$synthetic_blocks"
        printf 'Destination : node %s via %s\n' "$destination_node" "$relay_addr"
        if output="$(timeout --signal=TERM --kill-after=5s "${send_timeout_seconds}s" "$POLYGONE_BIN" envoyer --fichier "$payload" --dest "$destination_kem_pk" --via "$relay_addr" --a "$destination_node" 2>&1)"; then
            send_status=0
        else
            send_status=$?
        fi
        printf '%s\n' "$output"
        if (( send_status != 0 )); then
            if (( send_status == 124 || send_status == 137 )); then
                printf 'Échec : délai d’envoi expiré après %s s. timeout a envoyé SIGTERM, puis SIGKILL après 5 s si nécessaire.\n' "$send_timeout_seconds" >&2
                printf 'L’envoi est arrêté. Pour arrêter aussi l’écouteur ou le relais, faites Ctrl-C dans le terminal correspondant (rôles listen/relay).\n' >&2
            else
                printf 'Échec : la commande d’envoi s’est terminée avec le code %s.\n' "$send_status" >&2
            fi
            exit 1
        fi

        expected_hash=""
        while IFS= read -r line; do
            if [[ "$line" =~ BLAKE3[[:space:]]+([[:xdigit:]]{64})([^[:xdigit:]]|$) ]]; then
                expected_hash="${BASH_REMATCH[1],,}"
            fi
        done <<< "$output"
        if [[ -z "$expected_hash" ]]; then
            printf 'Erreur : aucun hash BLAKE3 exploitable (64 caractères hexadécimaux) dans la sortie d’envoi.\n' >&2
            exit 1
        fi

        if ! IFS= read -r -p 'Collez le hash BLAKE3 affiché par le destinataire (64 caractères hex) : ' received_hash; then
            printf '\nÉchec : aucun hash reçu fourni; comparaison non effectuée.\n' >&2
            exit 1
        fi
        if [[ ! "$received_hash" =~ ^[[:xdigit:]]{64}$ ]]; then
            printf 'Échec : le hash reçu doit contenir exactement 64 caractères hexadécimaux.\n' >&2
            exit 1
        fi
        if [[ "${received_hash,,}" != "$expected_hash" ]]; then
            printf 'FAIL : hash reçu %s différent du hash d’envoi %s.\n' "${received_hash,,}" "$expected_hash" >&2
            exit 1
        fi
        printf 'PASS : le BLAKE3 reçu correspond à celui du fichier envoyé (%s).\n' "$expected_hash"
        ;;
    *)
        usage >&2
        exit 2
        ;;
esac
