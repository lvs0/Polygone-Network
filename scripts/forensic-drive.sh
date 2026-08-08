#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════
#  forensic-drive.sh — prouve que le DRIVE tient sa promesse
#
#  Rejoue une session réelle de fichier (relay + 2 clients réels) :
#    1. Alice envoie un fichier chiffré + fragmenté (Shamir 4/7) via le
#       relay aveugle ;
#    2. Bob le reçoit dans ~/.polygone/received/ — contenu IDENTIQUE
#       (comparaison octet à octet) ;
#    3. le relay ne possède aucun fichier (stateless) — rien ne persiste.
#
#  Usage : scripts/forensic-drive.sh [port]
#  Sortie : OK + preuve, ou échec explicite. Exit 0 = preuve fournie.
# ═══════════════════════════════════════════════════════════════════════════
set -euo pipefail
PORT="${1:-7100}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="${POLYGONE_BIN:-$ROOT/target/release/polygone}"
RELAY_BIN="${RELAY_BIN:-$ROOT/target/release/polygone-relay}"

log()  { printf '%s\n' "✓ $*"; }
die()  { printf '%s\n' "✖ $*" >&2; exit 1; }

[ -x "$BIN" ] || die "client introuvable : $BIN (cargo build --release d'abord)"
[ -x "$RELAY_BIN" ] || die "relay introuvable : $RELAY_BIN (cargo build --release d'abord)"

TMP="$(mktemp -d)"
RELAY_PID=""
BOB_PID=""
trap 'kill ${RELAY_PID:-} ${BOB_PID:-} 2>/dev/null || true; rm -rf "$TMP"' EXIT
mkdir -p "$TMP/alice" "$TMP/bob" "$TMP/relay-cwd"

echo "⬡ FORENSIC DRIVE — la preuve que le fichier traverse et meurt"
echo "   relay : $RELAY_BIN (port $PORT)"

# ── 1. Relay réel, stateless (exec : $! = le vrai relay, tuable) ───────────
( cd "$TMP/relay-cwd" && exec "$RELAY_BIN" --port "$PORT" >"$TMP/relay.log" 2>&1 ) &
RELAY_PID=$!
sleep 0.5

# ── 2. Bob : identité + écoute ──────────────────────────────────────────────
BOB_PK="$(HOME="$TMP/bob" "$BIN" clef 2>/dev/null | head -1)"
[ -n "$BOB_PK" ] || die "Bob n'a pas d'identité"
BOB_NODE="${BOB_PK:0:16}"
HOME="$TMP/bob" "$BIN" ecouter --relay "127.0.0.1:$PORT" >"$TMP/bob.log" 2>&1 &
BOB_PID=$!
sleep 0.6

# ── 3. Alice : un fichier secret traverse le relay ──────────────────────────
echo "Plan secret Polygone — ce fichier doit arriver identique." >"$TMP/plan.txt"
HOME="$TMP/alice" "$BIN" envoyer --via "127.0.0.1:$PORT" --a "$BOB_NODE" -d "$BOB_PK" \
    --fichier "$TMP/plan.txt" >/dev/null 2>&1 \
    || die "l'envoi du fichier a échoué"
sleep 1.5

# ── 4. Preuve : contenu identique ───────────────────────────────────────────
RECEIVED="$TMP/bob/.polygone/received/plan.txt"
if [ ! -f "$RECEIVED" ]; then
    echo "── log de Bob ──"
    tail -5 "$TMP/bob.log"
    die "Bob n'a rien reçu — le drive n'a pas traversé"
fi
if cmp -s "$TMP/plan.txt" "$RECEIVED"; then
    log "Fichier reçu par Bob : contenu IDENTIQUE (octet à octet)"
else
    die "contenu différent — le drive a corrompu le fichier"
fi

# ── 5. Preuve : le relay n'a rien gardé ─────────────────────────────────────
files="$(find "$TMP/relay-cwd" -type f | wc -l)"
[ "$files" -eq 0 ] || die "le relay a créé $files fichier(s) — violation zero-persistance"
log "Relay stateless : 0 fichier après une session de fichier complète"

echo
echo "═══ VERDICT : le fichier traverse, identique, et rien ne reste. ═══"
echo "   1. drive fonctionnel (contenu identique)   ✓"
echo "   2. relay stateless (0 fichier)             ✓"
exit 0
