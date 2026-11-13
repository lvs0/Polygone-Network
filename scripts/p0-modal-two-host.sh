#!/usr/bin/env bash
# P0 — preuve réseau sur DEUX HÔTES, sans second ordinateur personnel.
#
#   A (cette machine)  →  relais + B (conteneur Modal, autre noyau, autre horloge)
#
# Aucun des deux pairs n'est un thread du même processus, et aucun n'est un
# namespace réseau du même noyau : A est cette machine, B est un conteneur dans
# un centre de données, joint par un tunnel TCP brut.
#
# Échoue si le message ne traverse pas un canal authentifié, si B n'apprend pas
# l'ancre de confiance de A, ou si le marqueur n'arrive pas.
#
# Ce que ce script ne prouve pas : la traversée de NAT résidentiel (aucun pair
# n'est derrière une box), ni le comportement sous perte de paquets prolongée.
set -Eeuo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="${POLYGONE_BIN_DIR:-$ROOT/target/debug}"
CLIENT="$BIN_DIR/polygone"
MARKER="P0MODAL-$(date -u +%Y%m%dT%H%M%SZ)-$RANDOM"
TMP="$(mktemp -d -t polygone-p0modal.XXXXXX)"
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

step "0. binaires et prérequis"
[[ -x "$CLIENT" && -x "$BIN_DIR/polygone-relay" ]] || fail "binaires absents dans $BIN_DIR"
command -v modal >/dev/null || fail "CLI modal absent"
printf 'client %s\nrelais %s\nmarqueur %s\n' "$CLIENT" "$BIN_DIR/polygone-relay" "$MARKER"

step "1. conteneur Modal : relais + nœud B"
modal run "$ROOT/scripts/p0-modal-two-host.py" --marker "$MARKER" >"$LOG" 2>&1 &
MPID=$!

tunnel_line=""
for _ in $(seq 1 900); do
  tunnel_line="$(grep -m1 '^TUNNEL=' "$LOG" 2>/dev/null || true)"
  [[ -n "$tunnel_line" ]] && break
  if ! kill -0 "$MPID" 2>/dev/null; then
    printf '%s\n' "$(tail -40 "$LOG")" >&2
    fail "le conteneur s'est arrêté avant d'ouvrir le tunnel"
  fi
  sleep 1
done
[[ -n "$tunnel_line" ]] || fail "aucun tunnel TCP ouvert (voir $LOG)"

TUN="${tunnel_line#TUNNEL=}"
HOST="${TUN% *}"
PORT="${TUN##* }"
B_ID="$(grep -m1 '^B_ID=' "$LOG" | cut -d= -f2-)"
B_KEM="$(grep -m1 '^B_KEM=' "$LOG" | cut -d= -f2-)"
[[ -n "$B_ID" && -n "$B_KEM" ]] || fail "identité de B absente du journal"
printf 'tunnel public : %s:%s\nB             : %s\nKEM           : %s… (%d caractères)\n' \
  "$HOST" "$PORT" "$B_ID" "${B_KEM:0:16}" "${#B_KEM}"

step "2. identité de A sur cette machine"
mkdir -p "$TMP/a"
A_ID="$(HOME="$TMP/a" "$CLIENT" id | grep -oE '[0-9a-f]{32}' | head -1)"
[[ -n "$A_ID" ]] || fail "identité de A illisible"
[[ "$A_ID" != "$B_ID" ]] || fail "A et B partagent la même identité"
printf 'A : %s\n' "$A_ID"

step "3. A envoie à B à travers le relais distant"
set +e
HOME="$TMP/a" timeout 120 "$CLIENT" envoyer --via "$HOST:$PORT" --a "$B_ID" -d "$B_KEM" "$MARKER" \
  | tee "$TMP/a.log"
SEND_RC=$?
set -e
grep -q "message envoyé" "$TMP/a.log" || fail "A n'a pas confirmé l'envoi (code $SEND_RC)"

step "4. verdict du conteneur"
wait "$MPID" || true
MPID=""

SEEN="$(grep -m1 '^B_SEEN_MARKER=' "$LOG" | cut -d= -f2-)"
ANCHOR="$(grep -m1 '^B_TRUST_ANCHOR=' "$LOG" | cut -d= -f2- || true)"
SUMMARY="$(grep -m1 '^SUMMARY=' "$LOG" | cut -d= -f2- || true)"
[[ "$SEEN" == "1" ]] || fail "B n'a pas reçu le marqueur"
grep -q "NOUVEAU PAIR APPRIS : $A_ID" <<<"$ANCHOR" ||
  fail "B n'a pas appris l'ancre de confiance de A — livraison non authentifiée"
printf 'marqueur reçu par B, ancre apprise : %s\n' "$ANCHOR"

step "5. deux hôtes, pas deux namespaces"
B_KERNEL="$(python3 -c 'import json,sys;d=json.loads(sys.argv[1]);print(d["b_kernel"])' "$SUMMARY")"
B_UPTIME="$(python3 -c 'import json,sys;d=json.loads(sys.argv[1]);print(d["b_uptime_clock"])' "$SUMMARY")"
B_BOOT="$(python3 -c 'import json,sys;d=json.loads(sys.argv[1]);print(d["b_boot_time"])' "$SUMMARY")"
A_UPTIME="$(cut -d' ' -f1 /proc/uptime)"
A_BOOT="$(awk '/^btime/{print $2}' /proc/stat)"
printf 'A : %s · uptime %ss · boot %s\n' "$(uname -sr)" "${A_UPTIME%.*}" "$A_BOOT"
printf 'B : %s · uptime %ss · boot %s\n' "$B_KERNEL" "${B_UPTIME%.*}" "$B_BOOT"
[[ "${A_BOOT}" != "${B_BOOT}" ]] || fail "les deux hôtes partagent la même date de démarrage — même noyau ?"

step "6. journal horodaté"
PROOF_DIR="$ROOT/docs/proofs"
mkdir -p "$PROOF_DIR"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
JOURNAL="$PROOF_DIR/modal-two-host-$STAMP.md"
cp "$TMP/a.log" "$PROOF_DIR/modal-two-host-$STAMP-a.log"
cp "$LOG" "$PROOF_DIR/modal-two-host-$STAMP-modal.log"
{
  echo "# Preuve réseau à deux hôtes — $STAMP (UTC)"
  echo
  echo "> Exécutée par \`scripts/p0-modal-two-host.sh\`. Journal brut :"
  echo "> [\`modal-two-host-$STAMP-a.log\`](modal-two-host-$STAMP-a.log) et"
  echo "> [\`modal-two-host-$STAMP-modal.log\`](modal-two-host-$STAMP-modal.log)."
  echo
  echo "## Montage"
  echo
  echo "| Élément | Valeur |"
  echo "|---|---|"
  echo "| Nœud A | cette machine, \`$(uname -sr)\`, uptime ${A_UPTIME%.*}s, boot $A_BOOT |"
  echo "| Nœud B | conteneur Modal \`ubuntu:24.04\`, \`$B_KERNEL\`, uptime ${B_UPTIME%.*}s, boot $B_BOOT |"
  echo "| Relais | dans le conteneur de B, exposé par \`modal.forward($PORT, unencrypted=True)\` |"
  echo "| Chemin | \`$HOST:$PORT\` (TCP brut, WAN) |"
  echo "| Identité A | \`$A_ID\` |"
  echo "| Identité B | \`$B_ID\` |"
  echo "| Clé KEM de B | ${#B_KEM} caractères hex |"
  echo "| Marqueur | \`$MARKER\` |"
  echo "| Code de retour du client A | ${SEND_RC} |"
  echo
  echo "## Résultat"
  echo
  echo "- Message arrivé chez B et marqueur retrouvé : **oui**."
  echo "- B a appris l'ancre de confiance de A : \`$ANCHOR\`."
  echo "- Les deux hôtes ont des dates de démarrage différentes (\`$A_BOOT\` ≠ \`$B_BOOT\`) :"
  echo "  ce ne sont pas deux namespaces du même noyau."
  echo
  echo "## Résumé machine"
  echo
  echo '```json'
  echo "$SUMMARY" | python3 -m json.tool 2>/dev/null || echo "$SUMMARY"
  echo '```'
  echo
  echo "## Ce que cette preuve n'établit pas"
  echo
  echo "- Traversée de NAT résidentiel : aucun des deux pairs n'est derrière une box."
  echo "- Résistance à la perte de paquets prolongée ou au changement d'adresse en cours de session."
  echo "- Anonymat : le relais voit les deux \`node_id\`, la session, les tailles et les horaires."
  echo "- \`modal.forward\` est marqué expérimental par Modal ; l'API peut changer."
} >"$JOURNAL"

printf 'journal : %s\n' "${JOURNAL#"$ROOT"/}"

step "PASS — deux hôtes distincts, hors loopback"
printf 'A %s (%s) → relais %s → B %s (%s)\n' "$A_ID" "$(uname -sr)" "$HOST:$PORT" "$B_ID" "$B_KERNEL"
