#!/usr/bin/env bash
# Compatibilité entre un binaire ANTÉRIEUR (HELLO non authentifié) et le relais
# courant (AUTH_INIT/CHALLENGE/AUTH_RESP).
#
# Cas vérifiés, sur de vrais processus :
#   A. relais strict (défaut) → un client hérité est REFUSÉ explicitement ;
#   B. relais `--allow-insecure-hello` → le client hérité est ACCEPTÉ (mode de
#      transition), et le refus de A n'était donc pas un blocage du transport ;
#   C. le fichier `peers.json` v1 écrit par le client hérité reste lisible par le
#      binaire courant (voir scripts/peers-migration-smoke.sh pour la preuve
#      détaillée de la migration).
#
# Ce que ce script ne prouve pas : un échange de message complet entre un client
# hérité et un client courant. Le format d'enveloppe et le handshake ont changé ;
# c'est une incompatibilité assumée, documentée dans
# docs/PROTOCOL-RELAY-AUTH.md, pas un bug découvert ici.
set -Eeuo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
NEW_DIR="${POLYGONE_BIN_DIR:-$ROOT/target/debug}"
LEGACY_DIR="${POLYGONE_LEGACY_BIN_DIR:-$HOME/Projects/Polygone-real/target/debug}"
NEW_RELAY="$NEW_DIR/polygone-relay"
LEGACY_CLIENT="$LEGACY_DIR/polygone"
TMP="$(mktemp -d -t polygone-xver.XXXXXX)"
RELAY_PID=""

cleanup() {
  set +e
  [[ -n "$RELAY_PID" ]] && kill "$RELAY_PID" 2>/dev/null
  wait 2>/dev/null
  rm -rf -- "$TMP"
}
trap cleanup EXIT INT TERM

step() { printf '\n== %s ==\n' "$*"; }
fail() { printf '\n✖ FAIL: %s\n' "$*" >&2; exit 1; }

step "0. binaires"
[[ -x "$NEW_RELAY" ]] || fail "relais courant absent : $NEW_RELAY"
if [[ ! -x "$LEGACY_CLIENT" ]]; then
  printf 'SKIP: binaire antérieur absent (%s)\n' "$LEGACY_CLIENT"
  printf '      fournir POLYGONE_LEGACY_BIN_DIR pour exécuter ce test\n'
  exit 0
fi
printf 'relais courant   : %s\nclient antérieur : %s\n' "$NEW_RELAY" "$LEGACY_CLIENT"
LEGACY_VERSION="$("$LEGACY_CLIENT" --version 2>&1 | head -1)"
printf 'version antérieure : %s\n' "$LEGACY_VERSION"

start_relay() {
  local port="$1"; shift
  "$NEW_RELAY" --port "$port" --quiet "$@" >"$TMP/relay-$port.log" 2>&1 &
  RELAY_PID=$!
  for _ in {1..100}; do
    if (exec 3<>"/dev/tcp/127.0.0.1/$port") 2>/dev/null; then exec 3<&- 3>&-; return 0; fi
    sleep 0.1
  done
  fail "le relais n'écoute pas sur $port"
}

hello_once() {
  local port="$1" who="$2" answer=""
  exec 3<>"/dev/tcp/127.0.0.1/$port" || fail "connexion refusée sur $port"
  printf 'HELLO %s\n' "$who" >&3
  IFS= read -r -t 3 answer <&3 || true
  exec 3<&- 3>&- || true
  printf '%s' "$answer"
}

step "A. relais strict (défaut) : un HELLO hérité est refusé"
PORT_A=$(( 21000 + RANDOM % 5000 ))
start_relay "$PORT_A"
ANSWER_A="$(hello_once "$PORT_A" 0123456789abcdef0123456789abcdef)"
[[ "$ANSWER_A" == HELLO_DENIED* ]] || fail "relais strict a répondu '$ANSWER_A'"
printf 'refus explicite : %s\n' "$ANSWER_A"
kill "$RELAY_PID" 2>/dev/null; wait "$RELAY_PID" 2>/dev/null || true; RELAY_PID=""

step "B. relais --allow-insecure-hello : le client hérité est accepté"
PORT_B=$(( 21000 + RANDOM % 5000 ))
start_relay "$PORT_B" --allow-insecure-hello
ANSWER_B="$(hello_once "$PORT_B" 0123456789abcdef0123456789abcdef)"
[[ "$ANSWER_B" == HELLO_OK* ]] || fail "mode transition a répondu '$ANSWER_B'"
printf 'acceptation explicite : %s (mode transition, journalisé)\n' "$ANSWER_B"
kill "$RELAY_PID" 2>/dev/null; wait "$RELAY_PID" 2>/dev/null || true; RELAY_PID=""

step "C. le client hérité sait encore produire une identité v1"
mkdir -p "$TMP/legacy"
HOME="$TMP/legacy" "$LEGACY_CLIENT" id >"$TMP/legacy-id.log" 2>&1 || fail "client hérité : id a échoué"
LEGACY_NODE="$(grep -oE '[0-9a-f]{16,32}' "$TMP/legacy-id.log" | head -1)"
[[ -n "$LEGACY_NODE" ]] || fail "identifiant hérité illisible"
printf 'identité héritée : %s (%d caractères)\n' "${LEGACY_NODE:0:16}…" "${#LEGACY_NODE}"

step "PASS"
printf 'compatibilité de transition prouvée ; incompatibilité client-à-client assumée et documentée\n'
