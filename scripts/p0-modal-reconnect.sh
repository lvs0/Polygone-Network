#!/usr/bin/env bash
# P0 — coupure du chemin public en pleine session, puis reprise.
#
#   A ──▶ tunnel 1 ──▶ R ──▶ B     (marqueur 1 livré)
#                 ✂ coupure
#   A ──▶ tunnel 2 ──▶ R ──▶ B     (B réenregistré, marqueur 2 livré)
#
# Échoue si B ne se réenregistre pas, s'il change d'identité, ou si le second
# message n'arrive pas.
set -Eeuo pipefail
trap 'rc=$?; printf "\n✖ ERREUR ligne %s (code %s) : %s\n" "$LINENO" "$rc" "$BASH_COMMAND" >&2' ERR

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="${POLYGONE_BIN_DIR:-$ROOT/target/debug}"
CLIENT="$BIN_DIR/polygone"
STAMP0="$(date -u +%Y%m%dT%H%M%SZ)"
M1="P0CUT1-$STAMP0-$RANDOM"
M2="P0CUT2-$STAMP0-$RANDOM"
TMP="$(mktemp -d -t polygone-p0cut.XXXXXX)"
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
[[ -x "$CLIENT" && -x "$BIN_DIR/polygone-relay" ]] || fail "binaires absents dans $BIN_DIR"
printf 'marqueur 1 : %s\nmarqueur 2 : %s\n' "$M1" "$M2"

step "1. conteneurs : relais R et nœud B"
modal run "$ROOT/scripts/p0-modal-reconnect.py" --marker1 "$M1" --marker2 "$M2" >"$LOG" 2>&1 &
MPID=$!
for _ in $(seq 1 900); do
  [[ -n "$(field ORCH_READY)" ]] && break
  if ! kill -0 "$MPID" 2>/dev/null; then tail -40 "$LOG" >&2; fail "arrêt avant préparation"; fi
  sleep 1
done
[[ -n "$(field ORCH_READY)" ]] || fail "les conteneurs ne sont pas prêts"

TUN1="$(field TUNNEL)"
ADDR1="${TUN1% *}:${TUN1##* }"
B_ID="$(field B_ID)"
B_KEM="$(field B_KEM)"
R_BOOT_ID="$(field R_BOOT_ID)"
B_BOOT_ID="$(field B_BOOT_ID)"
[[ -n "$ADDR1" && -n "$B_ID" && -n "$B_KEM" ]] || fail "informations initiales incomplètes"
printf 'tunnel 1 : %s\nB        : %s\n' "$ADDR1" "$B_ID"

step "2. identité de A"
mkdir -p "$TMP/a"
HOME="$TMP/a" "$CLIENT" id >"$TMP/a.id" 2>&1 || fail "polygone id a échoué : $(cat "$TMP/a.id")"
A_ID="$(grep -oE '[0-9a-f]{32}' "$TMP/a.id" | head -1 || true)"
[[ -n "$A_ID" && "$A_ID" != "$B_ID" ]] || fail "identité de A invalide"
printf 'A : %s\n' "$A_ID"
[[ "$R_BOOT_ID" != "$B_BOOT_ID" ]] || fail "R et B partagent le même boot_id"

step "3. premier message avant coupure"
set +e
HOME="$TMP/a" timeout 120 "$CLIENT" envoyer --via "$ADDR1" --a "$B_ID" -d "$B_KEM" "$M1" | tee "$TMP/a1.log"
RC1=$?
set -e
grep -q "message envoyé" "$TMP/a1.log" || fail "premier envoi refusé (code $RC1)"

step "4. coupure, réenregistrement, second message"
for _ in $(seq 1 600); do
  [[ -n "$(field B_LISTENING2)" ]] && break
  if ! kill -0 "$MPID" 2>/dev/null; then tail -40 "$LOG" >&2; fail "arrêt pendant la coupure"; fi
  sleep 1
done
[[ -n "$(field B_LISTENING2)" ]] || fail "B ne s'est pas réenregistré"
DIED="$(field B_LISTENER_DIED)"
TUN2="$(field TUNNEL2)"
ADDR2="${TUN2% *}:${TUN2##* }"
printf 'écouteur de B tombé à la coupure : %s\nnouveau tunnel : %s\n' "$DIED" "$ADDR2"

set +e
HOME="$TMP/a" timeout 120 "$CLIENT" envoyer --via "$ADDR2" --a "$B_ID" -d "$B_KEM" "$M2" | tee "$TMP/a2.log"
RC2=$?
set -e
grep -q "message envoyé" "$TMP/a2.log" || fail "second envoi refusé (code $RC2)"

step "5. verdict de B"
wait "$MPID" || true
MPID=""
SEEN1="$(field B_SEEN_MARKER1)"
SEEN2="$(field B_SEEN_MARKER2)"
ANCHOR="$(field B_TRUST_ANCHOR)"
B_SUMMARY="$(field B_SUMMARY)"
R_SUMMARY="$(field R_SUMMARY)"
[[ "$SEEN1" == "1" ]] || fail "marqueur 1 non reçu"
[[ "$SEEN2" == "1" ]] || fail "marqueur 2 non reçu après la coupure"
B_ID_AFTER="$(python3 -c 'import json,sys;print(json.loads(sys.argv[1])["b_id"])' "$B_SUMMARY")"
[[ "$B_ID_AFTER" == "$B_ID" ]] || fail "B a changé d'identité après la coupure"
printf 'marqueur 1 reçu : oui · marqueur 2 reçu : oui · identité inchangée : oui\n'
printf 'ancre : %s\n' "$ANCHOR"

step "6. journal"
PROOF_DIR="$ROOT/docs/proofs"
mkdir -p "$PROOF_DIR"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
JOURNAL="$PROOF_DIR/modal-reconnect-$STAMP.md"
cp "$TMP/a1.log" "$PROOF_DIR/modal-reconnect-$STAMP-a1.log"
cp "$TMP/a2.log" "$PROOF_DIR/modal-reconnect-$STAMP-a2.log"
cp "$LOG" "$PROOF_DIR/modal-reconnect-$STAMP-modal.log"
RELEARN="$(python3 -c 'import json,sys;print(json.loads(sys.argv[1])["b_anchor_relearned_after_cut"])' "$B_SUMMARY")"
{
  echo "# Coupure et reprise en pleine session — $STAMP (UTC)"
  echo
  echo "> Exécuté par \`scripts/p0-modal-reconnect.sh\`. Journaux bruts :"
  echo "> [\`modal-reconnect-$STAMP-a1.log\`](modal-reconnect-$STAMP-a1.log),"
  echo "> [\`modal-reconnect-$STAMP-a2.log\`](modal-reconnect-$STAMP-a2.log) et"
  echo "> [\`modal-reconnect-$STAMP-modal.log\`](modal-reconnect-$STAMP-modal.log)."
  echo
  echo "## Séquence"
  echo
  echo "| Étape | Ce qui a été fait | Résultat |"
  echo "|---|---|---|"
  echo "| 1 | A envoie \`$M1\` via le tunnel 1 (\`$ADDR1\`) | livré, code client \`$RC1\` |"
  echo "| 2 | le tunnel 1 est fermé par le relais | écouteur de B tombé : \`$DIED\` |"
  echo "| 3 | B se réenregistre sur le tunnel 2 (\`$ADDR2\`), même HOME, même identité | réenregistré |"
  echo "| 4 | A envoie \`$M2\` via le tunnel 2, même \`node_id\` et même clé KEM | livré, code client \`$RC2\` |"
  echo
  echo "| Élément | Valeur |"
  echo "|---|---|"
  echo "| Identité A | \`$A_ID\` |"
  echo "| Identité B | \`$B_ID\` (inchangée après la coupure) |"
  echo "| Ancre de confiance apprise avant la coupure | \`$ANCHOR\` |"
  echo "| Ancre réapprise après la coupure | \`$RELEARN\` |"
  echo "| boot_id de B | \`$B_BOOT_ID\` |"
  echo "| boot_id de R | \`$R_BOOT_ID\` |"
  echo
  echo "## Résultat"
  echo
  echo "- Marqueur 1 arrivé avant la coupure : **oui**."
  echo "- B s'est réenregistré **sans changer d'identité** : le \`node_id\`, la clé KEM et"
  echo "  l'ancre de confiance de A restent valides après la coupure."
  echo "- Marqueur 2 arrivé après la coupure : **oui**."
  echo
  echo "## Ce que cette preuve n'établit pas"
  echo
  echo "- La reprise si **le relais** redémarre : son état de slots est en mémoire, rien"
  echo "  n'est persisté. Ici seule la liaison publique a été coupée, pas le relais."
  echo "- La coupure réelle d'un lien résidentiel (changement d'IP publique, box qui"
  echo "  redémarre) : la coupure est provoquée par la fermeture du tunnel."
  echo "- La conservation des messages envoyés **pendant** la coupure : aucun n'a été"
  echo "  envoyé dans l'intervalle."
  echo "- \`modal.forward\` reste expérimental chez Modal."
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

step "PASS — coupure, réenregistrement, second message livré"
