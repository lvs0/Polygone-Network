#!/usr/bin/env bash
# P0 — premier parcours réseau réel, à processus indépendants :
#
#   polygone envoyer (HOME A)  →  polygone-relay  →  polygone ecouter (HOME B)
#
# Chaque pair a son propre HOME, donc sa propre identité ML-KEM-1024 +
# ML-DSA-65 : ce sont deux nœuds distincts, pas deux threads du même process.
# Le script échoue si le message ne traverse pas un canal authentifié.
#
# Ce que ce script prouve : handshake AUTH_INIT/CHALLENGE/AUTH_RESP/AUTH_OK
# entre trois processus, livraison réelle par le relais, ancre de confiance
# apprise côté B, et refus explicite d'un HELLO non authentifié.
# Ce qu'il ne prouve pas : le comportement sur deux machines distinctes
# (voir docs/P0-DEUX-MACHINES.md).
set -Eeuo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
CLIENT="$ROOT/target/debug/polygone"
RELAY="$ROOT/target/debug/polygone-relay"
MARKER="P0-$(date +%s)-$RANDOM"
TMP="$(mktemp -d -t polygone-p0.XXXXXX)"
RELAY_PID=""
LISTEN_PID=""

cleanup() {
  set +e
  [[ -n "$LISTEN_PID" ]] && kill "$LISTEN_PID" 2>/dev/null
  [[ -n "$RELAY_PID" ]] && kill "$RELAY_PID" 2>/dev/null
  wait 2>/dev/null
  rm -rf -- "$TMP"
}
trap cleanup EXIT INT TERM

step() { printf '\n== %s ==\n' "$*"; }
fail() { printf '\n✖ FAIL: %s\n' "$*" >&2; exit 1; }

step "0. binaires"
[[ -x "$CLIENT" ]] || cargo build --manifest-path "$ROOT/Cargo.toml" -p polygone-client
[[ -x "$RELAY" ]] || cargo build --manifest-path "$ROOT/Cargo.toml" -p polygone-relay
printf 'client %s\nrelais %s\n' "$CLIENT" "$RELAY"

PORT=$(( 20000 + RANDOM % 20000 ))
ADDR="127.0.0.1:$PORT"
mkdir -p "$TMP/a" "$TMP/b"

step "1. relais (processus 1) sur $ADDR"
"$RELAY" --port "$PORT" --quiet >"$TMP/relay.log" 2>&1 &
RELAY_PID=$!
ready=""
for _ in {1..100}; do
  if (exec 3<>"/dev/tcp/127.0.0.1/$PORT") 2>/dev/null; then exec 3<&- 3>&-; ready=1; break; fi
  sleep 0.1
done
[[ -n "$ready" ]] || fail "le relais n'écoute pas sur $ADDR (voir $TMP/relay.log)"
kill -0 "$RELAY_PID" 2>/dev/null || fail "le relais est mort au démarrage"

step "2. identités distinctes (deux HOME séparés)"
A_ID="$(HOME="$TMP/a" "$CLIENT" id | grep -oE '[0-9a-f]{32}' | head -1)"
B_ID="$(HOME="$TMP/b" "$CLIENT" id | grep -oE '[0-9a-f]{32}' | head -1)"
B_KEM="$(HOME="$TMP/b" "$CLIENT" clef | grep -oE '[0-9a-f]{3000,}' | head -1)"
[[ -n "$A_ID" && -n "$B_ID" && -n "$B_KEM" ]] || fail "identités illisibles"
[[ "$A_ID" != "$B_ID" ]] || fail "les deux pairs partagent la même identité"
printf 'A   = %s\nB   = %s\nKEM = %s… (%d caractères hex)\n' \
  "$A_ID" "$B_ID" "${B_KEM:0:16}" "${#B_KEM}"

step "3. B écoute (processus 2)"
HOME="$TMP/b" "$CLIENT" ecouter --relay "$ADDR" >"$TMP/b.log" 2>&1 &
LISTEN_PID=$!
sleep 1
kill -0 "$LISTEN_PID" 2>/dev/null || fail "B n'écoute pas (voir $TMP/b.log)"
grep -q "en écoute via relay $ADDR" "$TMP/b.log" || fail "B n'a pas confirmé son enregistrement"

step "4. A envoie via le relais (processus 3)"
HOME="$TMP/a" "$CLIENT" envoyer --via "$ADDR" --a "$B_ID" -d "$B_KEM" "$MARKER" | tee "$TMP/a.log"
grep -q "message envoyé" "$TMP/a.log" || fail "A n'a pas confirmé l'envoi (handshake refusé ?)"

step "5. livraison authentifiée chez B"
for _ in {1..100}; do grep -qF "$MARKER" "$TMP/b.log" && break; sleep 0.1; done
grep -qF "$MARKER" "$TMP/b.log" || fail "B n'a pas reçu le message (voir $TMP/b.log)"
grep -q "NOUVEAU PAIR APPRIS : $A_ID" "$TMP/b.log" ||
  fail "B n'a pas appris l'ancre de confiance de A — la livraison n'est pas authentifiée"
printf 'message reçu, ancre de confiance apprise pour %s\n' "$A_ID"

step "6. une identité non prouvée est refusée (HELLO hérité)"
exec 3<>"/dev/tcp/127.0.0.1/$PORT"
printf 'HELLO %s\n' "$B_ID" >&3
answer=""
IFS= read -r -t 3 answer <&3 || true
exec 3<&- 3>&- || true
[[ "$answer" == HELLO_DENIED* ]] || fail "le relais a accepté un HELLO non authentifié : '$answer'"
printf 'refus explicite : %s\n' "$answer"

step "7. aucune clé privée dans les journaux"
for log in "$TMP/relay.log" "$TMP/a.log" "$TMP/b.log"; do
  if grep -qiE 'sk_hex|sign_sk|kem_sk|AUTH_RESP [0-9a-f]{100}' "$log"; then
    fail "clé privée ou preuve d'authentification écrite dans $log"
  fi
done
printf 'aucun secret dans les trois journaux\n'

step "PASS"
printf 'A %s → relais %s → B %s\n' "$A_ID" "$ADDR" "$B_ID"
