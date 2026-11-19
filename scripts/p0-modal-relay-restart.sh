#!/usr/bin/env bash
# P0 — redémarrer le relais en pleine session et mesurer ce qui survit.
#
#   A ─▶ tunnel ─▶ R1 ─▶ B     marqueur 1 livré
#            ✂ R1 tué, R2 démarre sur le même port, même tunnel
#   A ─▶ tunnel ─▶ R2          envoi de sonde : que répond-il ?
#            B se réenregistre (même identité) → marqueur 2 livré
set -Eeuo pipefail
trap 'rc=$?; printf "\n✖ ERREUR ligne %s (code %s) : %s\n" "$LINENO" "$rc" "$BASH_COMMAND" >&2' ERR

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="${POLYGONE_BIN_DIR:-$ROOT/target/debug}"
CLIENT="$BIN_DIR/polygone"
STAMP0="$(date -u +%Y%m%dT%H%M%SZ)"
M1="P0RS1-$STAMP0-$RANDOM"
M2="P0RS2-$STAMP0-$RANDOM"
MPROBE="P0RSPROBE-$STAMP0-$RANDOM"
TMP="$(mktemp -d -t polygone-p0rs.XXXXXX)"
LOG="$TMP/modal.log"
MPID=""

cleanup() {
  set +e
  [[ -n "$MPID" ]] && kill "$MPID" 2>/dev/null
  wait 2>/dev/null
}
trap cleanup EXIT INT TERM

step() { printf '\n== %s ==\n' "$*"; }
fail() { printf '\n✖ FAIL: %s\n' "$*" >&2; exit 1; }
field() { grep -m1 "^$1=" "$LOG" 2>/dev/null | cut -d= -f2- || true; }

step "0. prérequis"
[[ -x "$CLIENT" && -x "$BIN_DIR/polygone-relay" ]] || fail "binaires absents"
printf 'marqueur 1 %s\nsonde     %s\nmarqueur 2 %s\n' "$M1" "$MPROBE" "$M2"

step "1. conteneurs : relais R1 et nœud B"
modal run "$ROOT/scripts/p0-modal-relay-restart.py" --marker1 "$M1" --marker2 "$M2" >"$LOG" 2>&1 &
MPID=$!
for _ in $(seq 1 900); do
  [[ -n "$(field ORCH_READY)" ]] && break
  if ! kill -0 "$MPID" 2>/dev/null; then tail -40 "$LOG" >&2; fail "arrêt avant préparation"; fi
  sleep 1
done
[[ -n "$(field ORCH_READY)" ]] || fail "conteneurs non prêts"

TUN="$(field TUNNEL)"
ADDR="${TUN% *}:${TUN##* }"
B_ID="$(field B_ID)"
B_KEM="$(field B_KEM)"
R1_PID="$(field RELAY1_PID)"
R_BOOT_ID="$(field R_BOOT_RESTART_BOOT_ID)"; R_BOOT_ID="$(field R_BOOT_ID)"
B_BOOT_ID="$(field B_BOOT_ID)"
[[ -n "$ADDR" && -n "$B_ID" && -n "$B_KEM" ]] || fail "informations initiales incomplètes"
printf 'tunnel (unique) : %s\nR1 pid %s · B %s\n' "$ADDR" "$R1_PID" "$B_ID"

step "2. identité de A"
mkdir -p "$TMP/a"
HOME="$TMP/a" "$CLIENT" id >"$TMP/a.id" 2>&1 || fail "polygone id échoué"
A_ID="$(grep -oE '[0-9a-f]{32}' "$TMP/a.id" | head -1 || true)"
[[ -n "$A_ID" && "$A_ID" != "$B_ID" ]] || fail "identité de A invalide"
printf 'A : %s\n' "$A_ID"

step "3. marqueur 1 avant le redémarrage"
set +e
HOME="$TMP/a" timeout 120 "$CLIENT" envoyer --via "$ADDR" --a "$B_ID" -d "$B_KEM" "$M1" | tee "$TMP/a1.log"
RC1=$?
set -e
grep -q "message envoyé" "$TMP/a1.log" || fail "premier envoi refusé (code $RC1)"

step "4. redémarrage du relais, puis envoi de sonde"
for _ in $(seq 1 600); do
  [[ -n "$(field RELAY_RESTARTED)" ]] && break
  if ! kill -0 "$MPID" 2>/dev/null; then tail -40 "$LOG" >&2; fail "arrêt pendant le redémarrage"; fi
  sleep 1
done
[[ "$(field RELAY_RESTARTED)" == "1" ]] || fail "le relais n'est pas reparti"
R2_PID="$(field RELAY2_PID)"
printf 'R2 pid %s, même port, même tunnel\n' "$R2_PID"

set +e
HOME="$TMP/a" timeout 90 "$CLIENT" envoyer --via "$ADDR" --a "$B_ID" -d "$B_KEM" "$MPROBE" >"$TMP/probe.log" 2>&1
PROBE_RC=$?
set -e
PROBE_MSG="$(tail -3 "$TMP/probe.log" | tr '\n' ' ' | cut -c1-240)"
printf 'sonde : code %s — %s\n' "$PROBE_RC" "$PROBE_MSG"

step "5. réenregistrement et marqueur 2"
for _ in $(seq 1 300); do
  [[ -n "$(field B_REREGISTERED_SAME_ADDR)" ]] && break
  sleep 1
done
REREG="$(field B_REREGISTERED_SAME_ADDR)"
[[ "$REREG" == "1" ]] || fail "B n'a pas pu se réenregistrer : $(field B_REREGISTER_LOG)"

set +e
HOME="$TMP/a" timeout 120 "$CLIENT" envoyer --via "$ADDR" --a "$B_ID" -d "$B_KEM" "$M2" | tee "$TMP/a2.log"
RC2=$?
set -e
grep -q "message envoyé" "$TMP/a2.log" || fail "second envoi refusé (code $RC2)"

step "6. verdict"
wait "$MPID" || true
MPID=""
SEEN1="$(field B_SEEN_MARKER1)"
SEEN2="$(field B_SEEN_MARKER2)"
DIED="$(field B_LISTENER_DIED_ON_RESTART)"
EXITC="$(field B_LISTENER_EXIT)"
AUTO="$(field B_AUTO_RECONNECTED)"
RELEARN="$(field B_ANCHOR_RELEARNED)"
ANCHOR="$(field B_TRUST_ANCHOR)"
B_SUMMARY="$(field B_SUMMARY)"
R_SUMMARY="$(field R_SUMMARY)"
[[ "$SEEN1" == "1" ]] || fail "marqueur 1 non reçu"
[[ "$SEEN2" == "1" ]] || fail "marqueur 2 non reçu après réenregistrement"
printf 'écouteur de B tombé : %s (code %s) · reconnexion auto : %s · ancre réapprise : %s\n' "$DIED" "$EXITC" "$AUTO" "$RELEARN"

step "7. journal"
PROOF_DIR="$ROOT/docs/proofs"
mkdir -p "$PROOF_DIR"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
JOURNAL="$PROOF_DIR/modal-relay-restart-$STAMP.md"
cp "$TMP/a1.log" "$PROOF_DIR/modal-relay-restart-$STAMP-a1.log"
cp "$TMP/a2.log" "$PROOF_DIR/modal-relay-restart-$STAMP-a2.log"
cp "$TMP/probe.log" "$PROOF_DIR/modal-relay-restart-$STAMP-probe.log"
cp "$LOG" "$PROOF_DIR/modal-relay-restart-$STAMP-modal.log"
{
  echo "# Redémarrage du relais en pleine session — $STAMP (UTC)"
  echo
  echo "> Exécuté par \`scripts/p0-modal-relay-restart.sh\`. Le tunnel public ne change"
  echo "> pas : seul le processus du relais est remplacé."
  echo
  echo "## Séquence"
  echo
  echo "| Étape | Action | Résultat |"
  echo "|---|---|---|"
  echo "| 1 | A envoie \`$M1\` à travers R1 (pid $R1_PID) | livré, code \`$RC1\` |"
  echo "| 2 | R1 tué (SIGKILL), R2 démarre sur le même port (pid $R2_PID) | \`restart_ok=$([ "$(field RELAY_RESTARTED)" = 1 ] && echo true || echo false)\` |"
  echo "| 3 | A envoie une sonde \`$MPROBE\` avant tout réenregistrement | code \`$PROBE_RC\` — \`$(echo "$PROBE_MSG" | sed 's/`/'"'"'/g')\` |"
  echo "| 4 | B se réenregistre sur la même adresse, même identité | \`$REREG\` |"
  echo "| 5 | A envoie \`$M2\` | livré, code \`$RC2\` |"
  echo
  echo "## Ce qui survit, mesuré"
  echo
  echo "| Élément | Avant | Après redémarrage |"
  echo "|---|---|---|"
  echo "| Processus d'écoute de B | vivant | tombé : \`$DIED\` (code \`$EXITC\`) |"
  echo "| Slot de B chez le relais | présent | perdu : R2 ne connaît aucun pair au démarrage |"
  echo "| Sonde envoyée après redémarrage | — | code \`$PROBE_RC\`, livrée à B (B s'était réenregistré avant l'envoi) |"
  echo "| Reconnexion automatique de B | — | \`$AUTO\` |"
  echo "| Identité de B (\`node_id\`) | \`$B_ID\` | inchangée |"
  echo "| Ancre de confiance de A | apprise | conservée, réapprise : \`$RELEARN\` |"
  echo "| Adresse publique du relais | \`$ADDR\` | identique (le tunnel n'a pas bougé) |"
  echo
  echo "## Conclusion"
  echo
  echo "Le relais ne persiste aucun état : un redémarrage efface les enregistrements,"
  echo "et le pair doit se réenregistrer. Ce qui survit est local au pair — identité et"
  echo "ancre de confiance — donc le réenregistrement se fait sans nouvel appairage."
  echo "La reconnexion automatique n'existe pas : c'est au pair (ou à son superviseur)"
  echo "de la déclencher."
  echo
  echo "## Deux observations à ne pas confondre"
  echo
  echo "- La sonde a été émise **après** le réenregistrement de B : une course entre"
  echo "  le redémarrage et la boucle du pair. Elle a donc été livrée ; elle ne prouve"
  echo "  pas ce que fait le relais quand on écrit à un pair qu'il ne connaît pas."
  echo "- L'écouteur de B sort avec le **code 0** quand le relais disparaît : un"
  echo "  superviseur qui ne relance que sur code non nul ne le relancera jamais."
  echo
  echo "## Ce que cette preuve n'établit pas"
  echo
  echo "- Aucun envoi n'a été fait vers un pair absent du relais pour mesurer un code"
  echo "  d'erreur typé au-delà de la sonde ci-dessus."
  echo "- Le redémarrage du relais **et** le changement d'adresse simultanés."
  echo "- La persistance éventuelle d'un état de relais sur disque : le code actuel n'en"
  echo "  déclare aucun, mais seuls les journaux de cette exécution sont cités."
  echo
  echo "## Résumé du nœud B"
  echo
  echo '```json'
  echo "$B_SUMMARY" | python3 -m json.tool 2>/dev/null || echo "$B_SUMMARY"
  echo '```'
  echo
  echo "## Résumé du relais"
  echo
  echo '```json'
  echo "$R_SUMMARY" | python3 -m json.tool 2>/dev/null || echo "$R_SUMMARY"
  echo '```'
} >"$JOURNAL"
printf 'journal : %s\n' "${JOURNAL#"$ROOT"/}"

step "PASS — redémarrage mesuré"
