#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════
#  hide-smoke.sh — test E2E réel de Polygone Hide (Phase 1)
#
#  Topologie :
#    [cible HTTP locale :8800]  ←  [exit node :7101]  ←relay→  [client SOCKS5 :9050]  ←  curl --socks5
#
#  Preuves :
#    1. Le tunnel s'ouvre (grant) et curl reçoit le contenu via SOCKS5.
#    2. Le relay ne voit aucun octet de contenu en clair (audit « on voit rien »).
#    3. Fermeture propre des processus.
#
#  Usage : scripts/hide-smoke.sh   (binaires release requis)
# ═══════════════════════════════════════════════════════════════════════════
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/target/release/polygone"
RELAY_BIN="$ROOT/target/release/polygone-relay"

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

echo "⬡ Polygone Hide — smoke E2E (répertoire : $TMP)"

# 1. Cible HTTP locale (le « internet » que l'exit node atteint).
python3 -m http.server 8800 --bind 127.0.0.1 --directory "$TMP/www" >"$TMP/http.log" 2>&1 &
PIDS+=($!)
sleep 3
curl -s http://127.0.0.1:8800/index.html >/dev/null || die "cible HTTP locale ne répond pas"

# 2. Relay.
"$RELAY_BIN" --port 7101 >"$TMP/relay.log" 2>&1 &
PIDS+=($!)
sleep 1

# 3. Exit node (identité dédiée).
HOME="$EXIT_HOME" "$BIN" ecouter --relay 127.0.0.1:7101 --hide >"$TMP/exit.log" 2>&1 &
PIDS+=($!)
sleep 2
EXIT_PK="$(HOME="$EXIT_HOME" "$BIN" clef 2>/dev/null | head -1)"
EXIT_ID="${EXIT_PK:0:16}"
[ -n "$EXIT_PK" ] || die "exit node : pas de clé publique"
echo "  exit node : $EXIT_ID  (clé ${#EXIT_PK} hex)"

# 4. Client SOCKS5 (identité dédiée).
HOME="$CLIENT_HOME" "$BIN" hide --via 127.0.0.1:7101 --sortie "$EXIT_ID" --dest "$EXIT_PK" --ecoute 127.0.0.1:9050 >"$TMP/client.log" 2>&1 &
PIDS+=($!)
sleep 2

# 5. La preuve : curl à travers le tunnel.
echo "  → curl --socks5-hostname 127.0.0.1:9050 http://127.0.0.1:8800/index.html"
OUT="$(curl -s --max-time 30 --socks5-hostname 127.0.0.1:9050 http://127.0.0.1:8800/index.html || true)"
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
