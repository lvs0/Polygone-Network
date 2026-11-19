#!/usr/bin/env bash
# P0 — preuve réseau sur TROIS noyaux distincts, sans second ordinateur personnel.
#
#   A (cette machine) ──WAN──▶ R (conteneur Modal n°1, relais)
#   B (conteneur Modal n°2) ──WAN──▶ R
#
# R et B sont deux conteneurs distincts : deux instances de noyau, deux horloges,
# deux piles réseau, deux `boot_id`. Aucun namespace n'est partagé.
#
# Échoue si le message ne traverse pas un canal authentifié, si B n'apprend pas
# l'ancre de confiance de A, ou si les trois hôtes ne se distinguent pas.
#
# Ce que ce script ne prouve pas : la traversée de NAT résidentiel (A est le seul
# hôte hors centre de données, et aucune box n'est dans le chemin), la résistance
# à la perte de paquets, ni l'anonymat.
set -Eeuo pipefail
trap 'rc=$?; printf "\n\u2716 ERREUR ligne %s (code %s) : %s\n" "$LINENO" "$rc" "$BASH_COMMAND" >&2' ERR

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="${POLYGONE_BIN_DIR:-$ROOT/target/debug}"
CLIENT="$BIN_DIR/polygone"
MARKER="P0THREE-$(date -u +%Y%m%dT%H%M%SZ)-$RANDOM"
TMP="$(mktemp -d -t polygone-p0three.XXXXXX)"
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

step "0. binaires et prérequis"
[[ -x "$CLIENT" && -x "$BIN_DIR/polygone-relay" ]] || fail "binaires absents dans $BIN_DIR"
command -v modal >/dev/null || fail "CLI modal absent"
printf 'client %s\nmarqueur %s\n' "$CLIENT" "$MARKER"

step "1. deux conteneurs Modal : relais (R) et nœud B"
modal run "$ROOT/scripts/p0-modal-three-kernels.py" --marker "$MARKER" >"$LOG" 2>&1 &
MPID=$!

for _ in $(seq 1 900); do
  [[ -n "$(field ORCH_READY)" ]] && break
  if ! kill -0 "$MPID" 2>/dev/null; then
    tail -40 "$LOG" >&2
    fail "l'application s'est arrêtée avant que les deux conteneurs soient prêts"
  fi
  sleep 1
done
[[ -n "$(field ORCH_READY)" ]] || fail "les conteneurs ne sont pas prêts (voir $LOG)"

TUN="$(field TUNNEL)"
HOST="${TUN% *}"
PORT="${TUN##* }"
B_ID="$(field B_ID)"
B_KEM="$(field B_KEM)"
R_IP="$(field R_LOCAL_IP)"
B_IP="$(field B_LOCAL_IP)"
R_BOOT="$(field R_BOOT_TIME)"
B_BOOT="$(field B_BOOT_TIME)"
R_BOOT_ID="$(field R_BOOT_ID)"
B_BOOT_ID="$(field B_BOOT_ID)"
R_KERNEL="$(field R_KERNEL)"
B_KERNEL="$(field B_KERNEL)"
[[ -n "$HOST" && -n "$B_ID" && -n "$B_KEM" ]] || fail "informations de B absentes du journal"
printf 'R  %s  ip %s  boot %s\n' "$R_KERNEL" "$R_IP" "$R_BOOT"
printf 'B  %s  ip %s  boot %s\n' "$B_KERNEL" "$B_IP" "$B_BOOT"
printf 'tunnel public : %s (relais R)\n' "$TUN"

step "2. identité de A sur cette machine"
mkdir -p "$TMP/a"
HOME="$TMP/a" "$CLIENT" id >"$TMP/a.id" 2>&1 || fail "polygone id a échoué sur A : $(cat "$TMP/a.id")"
A_ID="$(grep -oE '[0-9a-f]{32}' "$TMP/a.id" | head -1 || true)"
[[ -n "$A_ID" ]] || fail "identité de A illisible : $(cat "$TMP/a.id")"
[[ "$A_ID" != "$B_ID" ]] || fail "A et B partagent la même identité"
A_IP="$(ip -4 -o addr show scope global 2>/dev/null | awk 'NR==1{split($4,a,"/"); print a[1]}' || true)"
A_IP="${A_IP:-inconnue}"
A_BOOT="$(awk '/^btime/{print $2}' /proc/stat)"
A_BOOT_ID="$(cat /proc/sys/kernel/random/boot_id 2>/dev/null || echo inconnu)"
printf 'A  %s  ip %s  boot %s\n' "$(uname -sr)" "$A_IP" "$A_BOOT"

step "3. les trois hôtes sont distincts"
[[ -n "$R_BOOT_ID" && -n "$B_BOOT_ID" ]] || fail "boot_id manquant côté conteneur"
[[ "$A_BOOT_ID" != "$R_BOOT_ID" ]] || fail "A et R partagent le même boot_id — même noyau ?"
[[ "$A_BOOT_ID" != "$B_BOOT_ID" ]] || fail "A et B partagent le même boot_id — même noyau ?"
[[ "$R_BOOT_ID" != "$B_BOOT_ID" ]] || fail "R et B partagent le même boot_id — même conteneur ?"
printf 'trois noyaux distincts (boot_id) :\n  A %s\n  R %s\n  B %s\n' "$A_BOOT_ID" "$R_BOOT_ID" "$B_BOOT_ID"

step "4. A envoie à B à travers le relais R (deux sauts de WAN)"
set +e
HOME="$TMP/a" timeout 120 "$CLIENT" envoyer --via "$HOST:$PORT" --a "$B_ID" -d "$B_KEM" "$MARKER" \
  | tee "$TMP/a.log"
SEND_RC=$?
set -e
grep -q "message envoyé" "$TMP/a.log" || fail "A n'a pas confirmé l'envoi (code $SEND_RC)"

step "5. verdict des deux conteneurs"
wait "$MPID" || true
MPID=""

SEEN="$(field B_SEEN_MARKER)"
ANCHOR="$(field B_TRUST_ANCHOR)"
B_SUMMARY="$(field B_SUMMARY)"
R_SUMMARY="$(field R_SUMMARY)"
[[ "$SEEN" == "1" ]] || fail "B n'a pas reçu le marqueur"
grep -q "NOUVEAU PAIR APPRIS : $A_ID" <<<"$ANCHOR" ||
  fail "B n'a pas appris l'ancre de confiance de A — livraison non authentifiée"
printf 'marqueur reçu par B, ancre apprise : %s\n' "$ANCHOR"

step "6. journal horodaté"
PROOF_DIR="$ROOT/docs/proofs"
mkdir -p "$PROOF_DIR"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
JOURNAL="$PROOF_DIR/modal-three-kernels-$STAMP.md"
cp "$TMP/a.log" "$PROOF_DIR/modal-three-kernels-$STAMP-a.log"
cp "$LOG" "$PROOF_DIR/modal-three-kernels-$STAMP-modal.log"
ELAPSED="$(python3 -c 'import json,sys;print(json.loads(sys.argv[1])["elapsed_s"])' "$B_SUMMARY" 2>/dev/null || echo "?")"
{
  echo "# Preuve réseau à trois noyaux — $STAMP (UTC)"
  echo
  echo "> Exécutée par \`scripts/p0-modal-three-kernels.sh\`. Journaux bruts :"
  echo "> [\`modal-three-kernels-$STAMP-a.log\`](modal-three-kernels-$STAMP-a.log) et"
  echo "> [\`modal-three-kernels-$STAMP-modal.log\`](modal-three-kernels-$STAMP-modal.log)."
  echo
  echo "## Montage"
  echo
  echo "| Rôle | Hôte | Noyau | boot_id | Démarrage |"
  echo "|---|---|---|---|---|"
  echo "| A (expéditeur) | machine locale | \`$(uname -sr)\` | \`$A_BOOT_ID\` | \`$A_BOOT\` |"
  echo "| R (relais) | conteneur Modal n°1 | \`$R_KERNEL\` | \`$R_BOOT_ID\` | \`$R_BOOT\` |"
  echo "| B (destinataire) | conteneur Modal n°2 | \`$B_KERNEL\` | \`$B_BOOT_ID\` | \`$B_BOOT\` |"
  echo
  echo "| Élément | Valeur |"
  echo "|---|---|"
  echo "| Tunnel public du relais | \`$HOST:$PORT\` (TCP brut) |"
  echo "| Identité A | \`$A_ID\` |"
  echo "| Identité B | \`$B_ID\` |"
  echo "| Clé KEM de B | ${#B_KEM} caractères hex |"
  echo "| Marqueur | \`$MARKER\` |"
  echo "| Code de retour du client A | \`$SEND_RC\` |"
  echo "| Durée côté B (enregistrement → marqueur) | ${ELAPSED} s |"
  echo
  echo "## Résultat"
  echo
  echo "- Marqueur retrouvé chez B : **oui**, via R, chaque saut traversant le WAN."
  echo "- B a appris l'ancre de confiance de A : \`$ANCHOR\`."
  echo "- Trois \`boot_id\` distincts : ce sont trois noyaux, pas trois namespaces."
  echo "- R et B sont deux conteneurs Modal séparés : le relais route sans lire la"
  echo "  charge utile, et B ne partage pas le noyau du relais."
  echo
  echo "## Résumé du relais"
  echo
  echo '```json'
  echo "$R_SUMMARY" | python3 -m json.tool 2>/dev/null || echo "$R_SUMMARY"
  echo '```'
  echo
  echo "## Résumé du nœud B"
  echo
  echo '```json'
  echo "$B_SUMMARY" | python3 -m json.tool 2>/dev/null || echo "$B_SUMMARY"
  echo '```'
  echo
  echo "## Ce que cette preuve n'établit pas"
  echo
  echo "- Traversée de NAT résidentiel : A est le seul hôte hors centre de données, et"
  echo "  aucune box n'est dans le chemin."
  echo "- Résistance à la perte de paquets prolongée, au changement d'adresse, à la"
  echo "  reconnexion inter-hôtes."
  echo "- Anonymat : R voit les deux \`node_id\`, la session, les tailles, les horaires et"
  echo "  les deux adresses publiques."
  echo "- R et B reposent sur la même technologie de sandbox (gVisor) : deux instances"
  echo "  distinctes, pas deux matériels distincts. Les deux conteneurs rapportent"
  echo "  \`127.0.0.1\` comme adresse locale, ce qui rend l'IP inutilisable comme preuve"
  echo "  de séparation — d'où le \`boot_id\`."
  echo "- \`modal.forward\` est marqué expérimental par Modal ; l'API peut changer."
} >"$JOURNAL"

printf 'journal : %s\n' "${JOURNAL#"$ROOT"/}"

step "PASS — trois noyaux distincts"
printf 'A %s (%s) → R %s → B %s (%s)\n' "$A_ID" "$(uname -sr)" "$HOST:$PORT" "$B_ID" "$B_KERNEL"
