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

# ── stdin — le message n'apparaît jamais dans l'historique shell ─────────
# kill-switch.md le recommande ; la promesse doit tourner : envoyer --stdin
# → recevoir - (round-trip sans aucun argument en clair).
if [ -x "$BIN" ]; then
  PK="$(HOME="$TMP/stdin" "$BIN" clef 2>/dev/null | head -1)"
  if printf 'message sensible — jamais dans l historique' \
      | HOME="$TMP/stdin" "$BIN" envoyer -d "$PK" --stdin 2>/dev/null \
      | HOME="$TMP/stdin" "$BIN" recevoir - 2>/dev/null \
      | grep -q "message sensible"; then
    echo "✓ polygone envoyer --stdin | recevoir - (round-trip, zéro arg en clair)"
  else
    echo "✖ le round-trip stdin ne tient pas la promesse"
    exit 1
  fi
fi

# ── Axiome 5 : la machine est la menace — duress détruit RÉELLEMENT ────────
# État éphémère complet (4 fichiers) → 0 après `duress --confirmer`.
DU="$TMP/duress"
mkdir -p "$DU/.polygone/received"
printf 'keys' >"$DU/.polygone/identity.json"
printf '{}'   >"$DU/.polygone/reputation.json"
printf '{}'   >"$DU/.polygone/peers.json"
printf 'data' >"$DU/.polygone/received/f.txt"
before="$(find "$DU/.polygone" -type f | wc -l)"
if ! HOME="$DU" "$BIN" duress --confirmer >/dev/null 2>&1; then
  echo "✖ duress a échoué (exit non nul)"
  exit 1
fi
after="$(find "$DU/.polygone" -type f 2>/dev/null | wc -l)"
if [ "$before" -eq 4 ] && [ "$after" -eq 0 ]; then
  echo "✓ polygone duress (destruction réelle : 4 → 0 fichiers)"
else
  echo "✖ duress n'a pas tout détruit (avant=$before, après=$after)"
  exit 1
fi

# ── polygoned — le 4e binaire du workspace (README) ──────────────────────
PD="$ROOT/target/release/polygoned"
if [ -x "$PD" ]; then
  if HOME="$TMP/pd" timeout 15 "$PD" doctor >"$TMP/pd.log" 2>&1; then
    echo "✓ polygoned doctor (diagnostics système, exit 0)"
  else
    echo "✖ polygoned doctor a échoué"
    tail -5 "$TMP/pd.log"
    exit 1
  fi
else
  echo "✖ polygoned introuvable — le 4e binaire promis ne tourne pas"
  exit 1
fi

echo
echo "═══ VERDICT : les commandes promues tiennent leurs verdicts. ═══"
exit 0
