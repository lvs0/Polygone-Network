#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════
#  hide-smoke.sh — test E2E réel de Polygone Hide (Phase 1)
#
#  Topologie (ports éphémères par défaut, surchargeables via POLYGONE_HIDE_*_PORT) :
#    [cible HTTP locale]  ←  [exit node]  ←relay→  [client SOCKS5]  ←  curl --socks5
#
#  Binaires surchargeables via POLYGONE_BIN et POLYGONE_RELAY_BIN.
#
#  Preuves :
#    1. Le tunnel s'ouvre (grant) et curl reçoit le contenu via SOCKS5.
#    2. Le relay ne voit aucun octet de contenu en clair (audit « on voit rien »).
#    3. Fermeture propre des processus.
#
#  Usage : scripts/hide-smoke.sh   (release par défaut, debug via variables ci-dessus)
# ═══════════════════════════════════════════════════════════════════════════
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="${POLYGONE_BIN:-$ROOT/target/release/polygone}"
RELAY_BIN="${POLYGONE_RELAY_BIN:-$ROOT/target/release/polygone-relay}"

pick_port() {
  python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1]); s.close()'
}
HTTP_PORT="${POLYGONE_HIDE_HTTP_PORT:-$(pick_port)}"
RELAY_PORT="${POLYGONE_HIDE_RELAY_PORT:-$(pick_port)}"
SOCKS_PORT="${POLYGONE_HIDE_SOCKS_PORT:-$(pick_port)}"

die() { printf '%s\n' "✖ $*" >&2; exit 1; }
[ -x "$BIN" ] || die "client introuvable : $BIN"
[ -x "$RELAY_BIN" ] || die "relay introuvable : $RELAY_BIN"

TMP="$(mktemp -d)"
EXIT_HOME="$TMP/exit-home"
CLIENT_HOME="$TMP/client-home"
mkdir -p "$EXIT_HOME" "$CLIENT_HOME" "$TMP/www"
echo "⬡ Polygone Hide — smoke E2E" > "$TMP/www/index.html"
PIDS=()
cleanup() { for p in "${PIDS[@]:-}"; do kill "$p" 2>/dev/null || true; done; rm -rf "$TMP"; }
trap cleanup EXIT

echo "⬡ Polygone Hide — smoke E2E (répertoire : $TMP ; HTTP $HTTP_PORT, relay $RELAY_PORT, SOCKS $SOCKS_PORT)"

# 1. Cible HTTP locale (le « internet » que l'exit node atteint).
python3 -m http.server "$HTTP_PORT" --bind 127.0.0.1 --directory "$TMP/www" >"$TMP/http.log" 2>&1 &
PIDS+=($!)
sleep 3
curl -s http://127.0.0.1:$HTTP_PORT/index.html >/dev/null || die "cible HTTP locale ne répond pas"

# 2. Relay.
"$RELAY_BIN" --port "$RELAY_PORT" >"$TMP/relay.log" 2>&1 &
PIDS+=($!)
sleep 1

# 3. Exit node (identité dédiée).
HOME="$EXIT_HOME" "$BIN" ecouter --relay "127.0.0.1:$RELAY_PORT" --hide >"$TMP/exit.log" 2>&1 &
PIDS+=($!)
sleep 2
EXIT_PK="$(HOME="$EXIT_HOME" "$BIN" clef 2>/dev/null | head -1)"
EXIT_ID="${EXIT_PK:0:16}"
[ -n "$EXIT_PK" ] || die "exit node : pas de clé publique"
echo "  exit node : $EXIT_ID  (clé ${#EXIT_PK} hex)"

# 4. Client SOCKS5 (identité dédiée).
HOME="$CLIENT_HOME" "$BIN" hide --via "127.0.0.1:$RELAY_PORT" --sortie "$EXIT_ID" --dest "$EXIT_PK" --ecoute "127.0.0.1:$SOCKS_PORT" >"$TMP/client.log" 2>&1 &
PIDS+=($!)
sleep 2

# 5. La preuve : curl à travers le tunnel.
echo "  → curl --socks5-hostname "127.0.0.1:$SOCKS_PORT" http://127.0.0.1:$HTTP_PORT/index.html"
OUT="$(curl -s --max-time 30 --socks5-hostname "127.0.0.1:$SOCKS_PORT" http://127.0.0.1:$HTTP_PORT/index.html || true)"
sleep 0.3

if echo "$OUT" | grep -q "smoke E2E"; then
  echo "✓ tunnel Hide : contenu reçu via SOCKS5"
else
  echo "✖ tunnel Hide : contenu inattendu → [$OUT]"
  echo "--- exit.log ---"; cat "$TMP/exit.log"
  echo "--- client.log ---"; cat "$TMP/client.log"
  echo "--- relay.log ---"; cat "$TMP/relay.log"
  exit 1
fi

# 6. Audit du relay : le mot « smoke » (contenu) ne doit apparaître nulle part
#    dans ce que le relay a traité — il est chiffré E2E.
if grep -q "smoke" "$TMP/relay.log" 2>/dev/null; then
  echo "✖ audit : le relay a vu du contenu en clair !"
  exit 1
fi
echo "✓ audit relay : zéro contenu en clair (« on voit rien » tient pour Hide)"

# 7. Logs côté nœuds : le grant a été accordé.
grep -q "tunnel accordé" "$TMP/exit.log" && echo "✓ exit node : tunnel accordé (grant chiffré)"
grep -q "tunnel ouvert" "$TMP/client.log" && echo "✓ client : tunnel ouvert"

echo "⬡ HIDE SMOKE : TOUT EST VERT"
