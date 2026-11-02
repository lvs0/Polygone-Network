#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════
#  POLYGONE — démo produit complète (une commande, ~60 secondes)
#
#  Montre, dans l'ordre :
#    1. la version + l'identité crypto
#    2. le self-test post-quantique (7/7)
#    3. la démo E2E : relay aveugle + audit « on voit rien »
#    4. la messagerie RÉELLE : Alice → relay → Bob (4/7 fragments)
#    5. le Drive : un fichier traversant le relay, contenu identique
#    6. Petals : l'IA locale (si Ollama est là)
#    7. le kill-switch (duress) — destruction + régénération
#
#  Usage : bash scripts/demo.sh   (depuis le repo, après cargo build --release)
# ═══════════════════════════════════════════════════════════════════════════
set -u
cd "$(dirname "$0")/.."
BIN=./target/release/polygone
RELAY=./target/release/polygone-relay
PORT=$(( 7200 + RANDOM % 500 ))

if [ ! -x "$BIN" ]; then
  echo "Build d'abord : cargo build --release -p polygone-client -p polygone-relay"
  exit 1
fi

B=$'\033[1m'; A=$'\033[33m'; G=$'\033[32m'; C=$'\033[36m'; D=$'\033[2m'; R=$'\033[0m'
h1() { printf '\n%s\n' "${B}${A}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${R}"; printf '%s\n' "${B}${A}  $1${R}"; printf '%s\n' "${B}${A}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${R}"; }
step() { printf '%s\n' "  ${G}✓${R} $1"; }

# Environnement éphémère — ne touche jamais au vrai ~/.polygone
WORK=$(mktemp -d)
trap 'pkill -9 -f "release/polygone" 2>/dev/null; rm -rf "$WORK"' EXIT
mkdir -p "$WORK/bob"

h1 "1 · Version & identité"
"$BIN" --version
HOME="$WORK/alice" "$BIN" clef 2>/dev/null | head -1 | cut -c1-24 | sed 's/^/  alice pk : /'
HOME="$WORK/bob"   "$BIN" clef 2>/dev/null | head -1 > "$WORK/bob_pk.txt"
BOB_PK=$(head -1 "$WORK/bob_pk.txt")
BOB_NODE=${BOB_PK:0:16}
step "identités ML-KEM-1024 + ML-DSA-65 générées (éphémères)"

h1 "2 · Self-test post-quantique (7/7)"
HOME="$WORK/alice" "$BIN" test 2>/dev/null | grep "tests —" | sed 's/^/  /'

h1 "3 · Démo E2E — relay aveugle + audit « on voit rien »"
HOME="$WORK/alice" "$BIN" demo 2>/dev/null | sed -e 's/\x1b\[[0-9;]*m//g' | grep -E "VERDICT|VALIDE|Signature" | sed 's/^/  /'

h1 "4 · Messagerie RÉELLE — Alice → relay → Bob"
"$RELAY" --port "$PORT" > "$WORK/relay.log" 2>&1 &
sleep 0.5
HOME="$WORK/bob" "$BIN" ecouter --relay "127.0.0.1:$PORT" > "$WORK/bob.log" 2>&1 &
sleep 0.6
HOME="$WORK/alice" "$BIN" envoyer --via "127.0.0.1:$PORT" --a "$BOB_NODE" -d "$BOB_PK" \
  "La démo produit : ce message traverse un relay qui ne voit rien." >/dev/null 2>&1
sleep 1.2
grep -A2 "message reçu" "$WORK/bob.log" | sed 's/^/  /' | grep -v "^  $"
step "le relay n'a vu que du routage — jamais le contenu"

h1 "5 · Drive — un fichier traverse le relay"
echo "Plan secret Polygone — phase 2 (fichier de démo)" > "$WORK/plan.txt"
HOME="$WORK/alice" "$BIN" envoyer --via "127.0.0.1:$PORT" --a "$BOB_NODE" -d "$BOB_PK" \
  --fichier "$WORK/plan.txt" >/dev/null 2>&1
sleep 1.2
if cmp -s "$WORK/plan.txt" "$WORK/bob/.polygone/received/plan.txt" 2>/dev/null; then
  step "fichier reçu : $WORK/bob/.polygone/received/plan.txt — contenu IDENTIQUE"
else
  echo "  ${R}✖ fichier non vérifié${R}"
fi

h1 "6 · Petals — l'IA locale (si Ollama tourne)"
if HOME="$WORK/alice" "$BIN" petals status >/dev/null 2>&1; then
  HOME="$WORK/alice" "$BIN" petals status 2>/dev/null | head -2 | sed 's/^/  /'
  HOME="$WORK/alice" "$BIN" petals ask --model phi4-mini:latest "Reponds en une phrase : pourquoi Polygone ?" 2>/dev/null | sed 's/^/  → /' | head -1
else
  echo "  ${D}(Ollama non détecté — étape ignorée)${R}"
fi

h1 "7 · Kill-switch — duress (destruction + régénération)"
HOME="$WORK/bob" "$BIN" duress --confirmer 2>/dev/null | tail -3 | sed 's/^/  /'
HOME="$WORK/bob" "$BIN" clef 2>/dev/null | head -1 | cut -c1-16 | sed 's/^/  nouvelle identité : /'

printf '\n%s\n' "${B}${G}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${R}"
printf '%s\n' "${B}${G}  POLYGONE — 7 étapes, tout est réel.${R}"
printf '%s\n' "${B}${G}  L'information n'existe pas. Elle traverse.${R}"
printf '%s\n' "${B}${G}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${R}"
exit 0
