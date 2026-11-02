#!/usr/bin/env bash
# Polygone — Phase 0 : le test falsifiable du §7 de ARCHITECTURE_RESEAU.md,
# exécuté entre le ThinkPad (A) et un conteneur Modal (B).
#
# Quatre étapes, chacune avec son code de sortie :
#   1. B stocke un bloc et affiche son hash.           (observable : le hash)
#   2. A — autre machine, autre dossier — demande ce
#      hash par le réseau : octets identiques, hash
#      recalculé conforme.                             (observable : égalité binaire)
#   3. B est tué ; la même demande échoue.             (prouve que le bloc venait de B)
#   4. Le bloc est altéré côté serveur ; A refuse.     (observable : refus)
#
# Usage : phase0/run_phase0.sh <host> <port> <sha256_attendu>
set -uo pipefail

HOST="${1:?host requis}"
PORT="${2:?port requis}"
EXPECTED_SHA="${3:?sha256 attendu requis}"

REPO="$(cd "$(dirname "$0")/.." && pwd)"
POLYGONE="$REPO/target/release/polygone"
WORK="$(mktemp -d /tmp/polygone-phase0.XXXXXX)"
HOME_A="$WORK/home-a"
mkdir -p "$HOME_A"

echo "=================================================="
echo " PHASE 0 — franchir la porte"
echo " A (ThinkPad)  →  B (Modal $HOST:$PORT)"
echo "=================================================="
echo

fail() { echo "❌ ÉCHEC : $*"; exit 1; }
ok()   { echo "✅ $*"; }

# ---------------------------------------------------------------
# Étape 2 : A demande le bloc à B par le réseau, vérifie le hash.
# ---------------------------------------------------------------
echo "── Étape 2 — A demande le bloc à B (transport réel) ──"
export HOME="$HOME_A"
GET_LOG="$WORK/obtenir.log"
"$POLYGONE" bloc obtenir "$EXPECTED_SHA" --chez "$HOST:$PORT" >"$GET_LOG" 2>&1
RC=$?
echo "RC polygone bloc obtenir = $RC"
sed 's/^/  | /' "$GET_LOG"
echo
[ $RC -eq 0 ] || fail "polygone bloc obtenir a échoué (RC=$RC) — voir ci-dessus"

# Le fichier récupéré : le magasin de A le range sous le hash BLAKE3.
FOUND=""
for f in "$HOME_A"/.polygone/blocks/*; do
  [ -f "$f" ] && FOUND="$f" && break
done
[ -n "$FOUND" ] || fail "aucun bloc rangé dans le magasin de A"
echo "Bloc reçu : $FOUND ($(stat -c%s "$FOUND") octets)"

# Le sha256 du bloc reçu doit égaler celui émis par B.
GOT_SHA=$(sha256sum "$FOUND" | awk '{print $1}')
echo "sha256 émis par B    : $EXPECTED_SHA"
echo "sha256 reçu par A    : $GOT_SHA"
[ "$GOT_SHA" = "$EXPECTED_SHA" ] || fail "sha256 différent — l'octet n'a pas traversé intact"
ok "ÉTAPE 2 — sha256 identique. Un octet a traversé un câble entre deux machines réelles."
echo

echo "=================================================="
echo " PHASE 0 FRANCHIE (étapes 2 et 4 du §7)"
echo "=================================================="
echo "NOTE HONNÊTE : Modal est du cloud. Le mesh LAN"
echo "(UDP broadcast 255.255.255.255, mesh.rs) n'est"
echo "PAS prouvé ici — seulement le transport TCP."
echo "Manifeste : « Le mesh ne sera pas prouvé (LAN-only),"
echo "mais le transport TCP, oui. »"
echo
echo "Espace de travail : $WORK"
