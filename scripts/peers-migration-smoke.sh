#!/usr/bin/env bash
# Preuve de bout en bout de la migration `peers.json` v1 → v2.
#
# Un vrai binaire, un vrai HOME temporaire, un vrai fichier v1 écrit à la main :
#   1. une identité est créée ;
#   2. on écrit un `peers.json` au format v1 (identifiant de 16 caractères) ;
#   3. `polygone verite` le lit ;
#   4. on vérifie que le fichier contient désormais les DEUX identifiants,
#      que la copie `peers.json.v1.bak` existe et que rien n'a été supprimé.
#
# Ce que ce script ne prouve pas : le comportement sur une installation réelle
# avec des pairs déjà connus (aucune n'existe sur cette machine).
set -Eeuo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="${POLYGONE_BIN_DIR:-$ROOT/target/debug}"
CLIENT="$BIN_DIR/polygone"
TMP="$(mktemp -d -t polygone-peers.XXXXXX)"
cleanup() { set +e; rm -rf -- "$TMP"; }
trap cleanup EXIT INT TERM

step() { printf '\n== %s ==\n' "$*"; }
fail() { printf '\n✖ FAIL: %s\n' "$*" >&2; exit 1; }

step "0. binaire"
[[ -x "$CLIENT" ]] || fail "binaire absent : $CLIENT (construire d'abord)"
printf 'client %s\n' "$CLIENT"

step "1. identité dans un HOME temporaire"
HOME="$TMP" "$CLIENT" id >"$TMP/id.log" 2>&1 || fail "création d'identité impossible"
[[ -f "$TMP/.polygone/identity.json" ]] || fail "identity.json absent"
NODE_ID="$(grep -oE '[0-9a-f]{32}' "$TMP/id.log" | head -1)"
[[ -n "$NODE_ID" ]] || fail "identifiant courant illisible"
printf 'identifiant courant : %s (32 caractères)\n' "$NODE_ID"

step "2. écrire un peers.json au format v1"
LEGACY_ID="0123456789abcdef"
PK="$(python3 -c "import json;print(json.load(open('$TMP/.polygone/identity.json'))['sign_pk_hex'])")"
[[ -n "$PK" ]] || fail "clé de signature illisible dans identity.json"
printf '{"%s":"%s"}' "$LEGACY_ID" "$PK" >"$TMP/.polygone/peers.json"
chmod 600 "$TMP/.polygone/peers.json"
printf 'v1 : %s → %s…\n' "$LEGACY_ID" "${PK:0:16}"

step "3. lire avec le binaire courant (polygone verite)"
HOME="$TMP" "$CLIENT" verite >"$TMP/verite.log" 2>&1 || fail "verite a échoué"
grep -q "peers.json" "$TMP/verite.log" || fail "peers.json non énuméré par verite"
COUNT="$(grep -c '→' "$TMP/verite.log" || true)"
[[ "$COUNT" -eq 2 ]] || fail "attendu 2 empreintes (v1 + dérivée), obtenu $COUNT"
printf '%s\n' "$(grep '→' "$TMP/verite.log")"

step "4. le fichier contient les deux identifiants, et la copie v1 existe"
python3 - "$TMP/.polygone/peers.json" "$LEGACY_ID" "$NODE_ID" <<'PY' || exit 1
import json, sys
path, legacy, current = sys.argv[1:4]
store = json.load(open(path))
if legacy not in store:
    sys.exit("l'identifiant v1 a disparu du fichier")
if current not in store:
    sys.exit("l'identifiant dérivé n'a pas été ajouté")
if store[legacy] != store[current]:
    sys.exit("les deux identifiants ne pointent pas vers la même clé")
print(f"fichier migré : {len(store)} entrées, dont v1 {legacy} et courant {current}")
PY
[[ -f "$TMP/.polygone/peers.json.v1.bak" ]] || fail "copie peers.json.v1.bak absente"
grep -q "$LEGACY_ID" "$TMP/.polygone/peers.json.v1.bak" || fail "la copie ne contient pas l'original"
if grep -q "$NODE_ID" "$TMP/.polygone/peers.json.v1.bak"; then
  fail "la copie contient déjà la version migrée : elle n'est pas l'original"
fi
printf 'copie v1 intacte : %s\n' "$(stat -c '%a' "$TMP/.polygone/peers.json.v1.bak")"

step "PASS"
printf 'migration v1 → v2 prouvée sur un fichier réel, sans perte d entrée\n'
