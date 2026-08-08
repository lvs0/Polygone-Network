#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════
#  smoke-commands.sh — les commandes produit tiennent leurs verdicts
#
#  Règle produit++ : chaque promesse du README est un test CI ou une
#  commande `polygone *`. Ici, les commandes promues sont EXÉCUTÉES sur
#  les binaires du commit courant et doivent se terminer (exit 0) :
#    1. `polygone test`        — self-test crypto 7/7
#    2. `polygone verite`      — forensique locale, « rien »
#    3. `polygone carte`       — la clé comme objet social
#    4. `polygone premier-soir` — le scénario guidé (TTL court)
#    5. `polygone demo`        — la démo E2E complète (~60 s)
#
#  Usage : scripts/smoke-commands.sh
#  Sortie : OK + verdict, ou échec explicite. Exit 0 = tout tient.
# ═══════════════════════════════════════════════════════════════════════════
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="${POLYGONE_BIN:-$ROOT/target/release/polygone}"

die() { printf '%s\n' "✖ $*" >&2; exit 1; }

[ -x "$BIN" ] || die "client introuvable : $BIN (cargo build --release d'abord)"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

check() { # <nom> <commande…>
  local name="$1"; shift
  if "$@" >/dev/null 2>&1; then
    printf '%s\n' "✓ $name"
  else
    printf '%s\n' "✖ $name a échoué (exit $?)"
    exit 1
  fi
}

echo "⬡ SMOKE — les commandes produit tournent sur le code du commit"
echo "   binaire : $BIN"

check "polygone test (7/7 crypto)"        env HOME="$TMP/a" "$BIN" test
check "polygone verite (forensique)"      env HOME="$TMP/a" "$BIN" verite
check "polygone carte (objet social)"      env HOME="$TMP/a" "$BIN" carte
check "polygone premier-soir --ttl 2"     env HOME="$TMP/b" "$BIN" premier-soir --ttl 2
check "polygone demo (E2E, ~60 s)"        env HOME="$TMP/c" timeout 150 "$BIN" demo

echo
echo "═══ VERDICT : les commandes promues tiennent leurs verdicts. ═══"
exit 0
