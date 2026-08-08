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

# ── wire.txt — la forme fichier du transport (README quickstart) ────────
# Le quickstart promet `envoyer … > wire.txt` puis `recevoir wire.txt` :
# le round-trip FICHIER (pas seulement stdin) doit tenir.
if [ -x "$BIN" ]; then
  PK2="$(HOME="$TMP/wire" "$BIN" clef 2>/dev/null | head -1)"
  if HOME="$TMP/wire" "$BIN" envoyer -d "$PK2" "message fichier wire" 2>/dev/null \
      >"$TMP/wire.txt" \
    && HOME="$TMP/wire" "$BIN" recevoir "$TMP/wire.txt" 2>/dev/null \
      | grep -q "message fichier wire"; then
    echo "✓ polygone envoyer > wire.txt | recevoir wire.txt (round-trip fichier)"
  else
    echo "✖ la forme fichier du quickstart ne tient pas"
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

# ── version — la promesse « Version : v2.0.0-rc2 » du README ─────────────
# Les binaires doivent afficher la version du workspace (Cargo.toml).
EXPECTED_VERSION="$(grep -m1 '^version' "$ROOT/Cargo.toml" | sed 's/.*"\(.*\)".*/\1/')"
for bin in "$BIN" "$ROOT/target/release/polygone-relay" "$ROOT/target/release/polygoned"; do
  if [ -x "$bin" ]; then
    actual="$("$bin" --version 2>/dev/null | awk '{print $2}')"
    if [ -n "$EXPECTED_VERSION" ] && [ "$actual" = "$EXPECTED_VERSION" ]; then
      printf '%s\n' "✓ $(basename "$bin") --version → $actual (== Cargo.toml)"
    else
      printf '%s\n' "✖ version incohérente : $(basename "$bin")='$actual', Cargo.toml='$EXPECTED_VERSION'"
      exit 1
    fi
  fi
done

# ── D8 — la config LEGACY se lit (rétro-compat) ─────────────────────────
# Une config écrite par une version antérieure ([tier] en table, section
# [platform] inconnue) doit charger et afficher SON tier — la preuve que
# la config est lue, pas le fallback default.
PD="$ROOT/target/release/polygoned"
LEGACY="$TMP/legacy"
mkdir -p "$LEGACY/.config/polygone"
cat >"$LEGACY/.config/polygone/daemon.toml" <<'TOML'
[tier]
tier = "Performance"

[safety]
min_free_ram_gb = 4.0
min_free_cpu_cores = 1
min_free_vram_mb = 512
max_cpu_percent = 85

[behavior]
grow_step_pct = 10
shrink_step_pct = 5
shrink_hysteresis_ticks = 5
throttle_on_user_activity = true
tick_interval_secs = 5

[platform]
mode = "linux"
TOML
if HOME="$LEGACY" timeout 15 "$PD" status 2>"$TMP/legacy.log" \
    | grep -qi "tier.*performance"; then
  echo "✓ polygoned lit la config legacy ([tier] en table) → tier Performance"
else
  echo "✖ la config legacy ne charge pas, ou son tier est perdu (default)"
  tail -5 "$TMP/legacy.log"
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

# ── socket honnête — status reflète le VRAI chemin du daemon ────────────
# Le daemon écrit ~/.polygone/daemon.sock ; l'indicateur doit suivre ce
# chemin : absent → ❌, présent → ✅ (jamais un chemin mort).
SO="$TMP/sock"
mkdir -p "$SO/.polygone"
if ! HOME="$SO" timeout 15 "$PD" status 2>/dev/null | grep -q "daemon.sock.*❌"; then
  echo "✖ status ne reflète pas le socket absent (❌ attendu)"
  exit 1
fi
touch "$SO/.polygone/daemon.sock"
if ! HOME="$SO" timeout 15 "$PD" status 2>/dev/null | grep -q "daemon.sock.*✅"; then
  echo "✖ status ne reflète pas le socket présent (✅ attendu)"
  exit 1
fi
echo "✓ polygoned status reflète le socket réel (absent → ❌, présent → ✅)"

# ── run-loop — la boucle d'allocation dynamique tourne et sort proprement ─
# La promesse centrale du daemon (allouer les ressources libres en temps
# réel) doit tourner sur le binaire réel : ticks visibles, SIGINT géré,
# sortie propre (exit 0, pas de kill brutal).
RUN="$TMP/runloop"
mkdir -p "$RUN/.config/polygone"
cat >"$RUN/.config/polygone/daemon.toml" <<'TOML'
tier = "Eco"

[behavior]
grow_step_pct = 10
shrink_step_pct = 5
shrink_hysteresis_ticks = 5
throttle_on_user_activity = true
tick_interval_secs = 1

[safety]
min_free_ram_gb = 4.0
min_free_cpu_cores = 1
min_free_vram_mb = 512
max_cpu_percent = 85
TOML
if HOME="$RUN" timeout -s INT --preserve-status 6 "$PD" --dry-run \
    >"$TMP/run.log" 2>&1 \
  && grep -q "starting on linux" "$TMP/run.log" \
  && grep -q "CPU:.*Alloc:" "$TMP/run.log" \
  && grep -q "exited cleanly" "$TMP/run.log"; then
  echo "✓ polygoned --dry-run : la boucle d'allocation tourne (ticks) et sort proprement"
else
  echo "✖ le run-loop du daemon ne tourne pas proprement"
  tail -5 "$TMP/run.log"
  exit 1
fi

echo
echo "═══ VERDICT : les commandes promues tiennent leurs verdicts. ═══"
exit 0
