#!/usr/bin/env bash
# P0 — parcours à DEUX HÔTES L3 distincts, hors loopback.
#
#   A (10.200.1.2)  ──veth──  R (10.200.1.1 / 10.200.2.1)  ──veth──  B (10.200.2.2)
#
# Trois piles réseau séparées par le noyau (namespaces) : A et B sont sur des
# sous-réseaux différents et ne peuvent pas se joindre directement — tout
# passe par la pile de R, où tourne le relais. Ce n'est pas du loopback.
#
# Honnêteté : c'est un test à deux hôtes **réseau**, sur un seul noyau et une
# seule machine physique. Il prouve le chemin L3, le routage, l'authentification
# et l'isolation ; il ne prouve pas ce qui dépend du matériel (horloges
# distinctes, carte réseau, perte réelle, NAT). Voir docs/P0-DEUX-MACHINES.md.
#
# Prérequis : root (création de namespaces) ; `ip` ; binaires construits par
# l'utilisateur avant l'appel :
#   cargo build -p polygone-client -p polygone-relay
#   sudo env POLYGONE_SKIP_BUILD=1 ./scripts/p0-two-host.sh
set -Eeuo pipefail

[[ "${EUID:-$(id -u)}" -eq 0 ]] || {
  echo "✖ ce test crée des namespaces réseau : lancez-le avec sudo" >&2
  exit 1
}

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
# Même règle que p0-two-process.sh : jamais de binaire périmé. Mais ce script
# tourne en root : `cargo` y hérite d'un HOME=/root et d'un cache sccache
# inutilisable. On ne construit donc jamais ici en root — l'appelant construit
# avec son propre compte, puis lance `sudo env POLYGONE_SKIP_BUILD=1 …`.
if [[ "${POLYGONE_SKIP_BUILD:-0}" != "1" && "${EUID:-$(id -u)}" -ne 0 ]]; then
  cargo build --manifest-path "$ROOT/Cargo.toml" -p polygone-client -p polygone-relay
fi
BIN_DIR="${POLYGONE_BIN_DIR:-$ROOT/target/debug}"
CLIENT="$BIN_DIR/polygone"
RELAY="$BIN_DIR/polygone-relay"
[[ -x "$CLIENT" && -x "$RELAY" ]] || {
  echo "✖ binaires absents dans $BIN_DIR (cargo build -p polygone-client -p polygone-relay)" >&2
  exit 1
}

NS_A="p0a$$"
NS_B="p0b$$"
NS_R="p0r$$"
PORT=7000
TMP="$(mktemp -d -t polygone-p0host.XXXXXX)"
MARKER="P0HOST-$(date +%s)-$RANDOM"
RELAY_PID=""
LISTEN_PID=""

cleanup() {
  set +e
  [[ -n "$LISTEN_PID" ]] && kill "$LISTEN_PID" 2>/dev/null
  [[ -n "$RELAY_PID" ]] && kill "$RELAY_PID" 2>/dev/null
  wait 2>/dev/null
  ip netns del "$NS_A" 2>/dev/null
  ip netns del "$NS_B" 2>/dev/null
  ip netns del "$NS_R" 2>/dev/null
  rm -rf -- "$TMP"
}
trap cleanup EXIT INT TERM

step() { printf '\n== %s ==\n' "$*"; }
fail() { printf '\n✖ FAIL: %s\n' "$*" >&2; exit 1; }

step "1. trois hôtes L3 (namespaces réseau)"
for ns in "$NS_A" "$NS_B" "$NS_R"; do ip netns add "$ns"; done
ip link add p0va type veth peer name p0vra
ip link set p0va netns "$NS_A"
ip link set p0vra netns "$NS_R"
ip link add p0vb type veth peer name p0vrb
ip link set p0vb netns "$NS_B"
ip link set p0vrb netns "$NS_R"

ip -n "$NS_A" addr add 10.200.1.2/24 dev p0va
ip -n "$NS_R" addr add 10.200.1.1/24 dev p0vra
ip -n "$NS_B" addr add 10.200.2.2/24 dev p0vb
ip -n "$NS_R" addr add 10.200.2.1/24 dev p0vrb

for ns in "$NS_A" "$NS_B" "$NS_R"; do
  ip -n "$ns" link set lo up
done
ip -n "$NS_A" link set p0va up
ip -n "$NS_B" link set p0vb up
ip -n "$NS_R" link set p0vra up
ip -n "$NS_R" link set p0vrb up
printf 'A  %s\nR  %s / %s\nB  %s\n' "10.200.1.2" "10.200.1.1" "10.200.2.1" "10.200.2.2"

step "2. isolation : A ne joint pas B en direct"
if ip -n "$NS_A" route get 10.200.2.2 >/dev/null 2>&1; then
  fail "A a une route directe vers B — le test ne prouverait pas le relais"
fi
printf 'aucune route de A vers 10.200.2.2 : le trafic doit passer par R\n'

step "3. relais sur R (hôte 3)"
ip netns exec "$NS_R" "$RELAY" --port "$PORT" --quiet >"$TMP/relay.log" 2>&1 &
RELAY_PID=$!
ready=""
for _ in {1..100}; do
  if ip netns exec "$NS_A" bash -c "exec 3<>/dev/tcp/10.200.1.1/$PORT" 2>/dev/null; then ready=1; break; fi
  sleep 0.1
done
[[ -n "$ready" ]] || fail "le relais de R n'est pas joignable depuis A (voir $TMP/relay.log)"

step "4. identités distinctes sur deux hôtes"
mkdir -p "$TMP/a" "$TMP/b"
chmod 755 "$TMP"
A_ID="$(ip netns exec "$NS_A" env HOME="$TMP/a" "$CLIENT" id | grep -oE '[0-9a-f]{32}' | head -1)"
B_ID="$(ip netns exec "$NS_B" env HOME="$TMP/b" "$CLIENT" id | grep -oE '[0-9a-f]{32}' | head -1)"
B_KEM="$(ip netns exec "$NS_B" env HOME="$TMP/b" "$CLIENT" clef | grep -oE '[0-9a-f]{3000,}' | head -1)"
[[ -n "$A_ID" && -n "$B_ID" && -n "$B_KEM" ]] || fail "identités illisibles"
[[ "$A_ID" != "$B_ID" ]] || fail "les deux hôtes partagent la même identité"
printf 'A = %s (10.200.1.2)\nB = %s (10.200.2.2)\n' "$A_ID" "$B_ID"

step "5. B écoute via R"
ip netns exec "$NS_B" env HOME="$TMP/b" "$CLIENT" ecouter --relay "10.200.2.1:$PORT" \
  >"$TMP/b.log" 2>&1 &
LISTEN_PID=$!
sleep 1
kill -0 "$LISTEN_PID" 2>/dev/null || fail "B n'écoute pas (voir $TMP/b.log)"

step "6. A envoie à B par R (sous-réseaux différents)"
ip netns exec "$NS_A" env HOME="$TMP/a" "$CLIENT" envoyer \
  --via "10.200.1.1:$PORT" --a "$B_ID" -d "$B_KEM" "$MARKER" | tee "$TMP/a.log"
grep -q "message envoyé" "$TMP/a.log" || fail "A n'a pas confirmé l'envoi"

step "7. livraison authentifiée sur l'autre hôte"
for _ in {1..100}; do grep -qF "$MARKER" "$TMP/b.log" && break; sleep 0.1; done
grep -qF "$MARKER" "$TMP/b.log" || fail "B n'a pas reçu le message (voir $TMP/b.log)"
grep -q "NOUVEAU PAIR APPRIS : $A_ID" "$TMP/b.log" || fail "ancre de confiance absente côté B"
printf 'message reçu et authentifié de %s\n' "$A_ID"

step "8. identité non prouvée refusée depuis A"
ip netns exec "$NS_A" bash -c "
  exec 3<>/dev/tcp/10.200.1.1/$PORT
  printf 'HELLO $B_ID\n' >&3
  IFS= read -r -t 3 answer <&3 || true
  exec 3<&-
  printf '%s' \"\$answer\"
" >"$TMP/hello.txt"
grep -q "^HELLO_DENIED" "$TMP/hello.txt" || fail "HELLO non authentifié accepté : $(cat "$TMP/hello.txt")"
printf 'refus explicite : %s\n' "$(cat "$TMP/hello.txt")"

step "9. aucune clé privée dans les journaux"
for log in "$TMP/relay.log" "$TMP/a.log" "$TMP/b.log"; do
  grep -qiE 'sk_hex|sign_sk|kem_sk|AUTH_RESP [0-9a-f]{100}' "$log" &&
    fail "clé privée ou preuve écrite dans $log" || true
done
printf 'aucun secret dans les trois journaux\n'

step "PASS — deux hôtes L3 distincts, hors loopback"
printf 'A %s (10.200.1.2) → R (10.200.1.1) → B %s (10.200.2.2)\n' "$A_ID" "$B_ID"
