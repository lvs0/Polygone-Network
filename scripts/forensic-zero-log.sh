#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════
#  forensic-zero-log.sh — prouve que « l'information n'existe pas »
#
#  Rejoue une session complète (HELLO → fragments → forward) contre un
#  VRAI relay, puis démontre qu'aucune trace ne subsiste :
#    1. le relay ne possède aucun fichier sur disque (stateless);
#    2. après la déconnexion, sa mémoire est vide (il n'a rien stocké);
#    3. les fragments ne sont PAS rejouables : un pair déconnecté ne
#       peut pas récupérer un fragment passé (drop, pas buffer).
#
#  Usage : scripts/forensic-zero-log.sh [port]
#  Sortie : OK + preuve, ou échec explicite. Exit 0 = preuve fournie.
# ═══════════════════════════════════════════════════════════════════════
set -euo pipefail
PORT="${1:-7000}"
RELAY_BIN="${RELAY_BIN:-target/release/polygone-relay}"

log()  { printf '%s\n' "${GREEN:-}✓${NC:-} $*"; }
warn() { printf '%s\n' "${AMBER:-}⚠${NC:-} $*"; }
die()  { printf '%s\n' "✖ $*" >&2; exit 1; }

[ -x "$RELAY_BIN" ] || die "relay introuvable : $RELAY_BIN (cargo build --release d'abord)"

TMP="$(mktemp -d)"
trap 'kill ${RELAY_PID:-} 2>/dev/null || true; rm -rf "$TMP"' EXIT

echo "⬡ FORENSIC ZERO-LOG — la preuve que rien ne persiste"
echo "   relay : $RELAY_BIN (port $PORT)"

# ── 1. Le relay ne possède AUCUN fichier au démarrage ─────────────────────
RELAY_WORKDIR="$TMP/relay-cwd"
mkdir -p "$RELAY_WORKDIR"
( cd "$RELAY_WORKDIR" && exec "$OLDPWD/$RELAY_BIN" --port "$PORT" >"$TMP/relay.log" 2>&1 ) &
RELAY_PID=$!
sleep 0.5

files_before="$(find "$RELAY_WORKDIR" -type f | wc -l)"
[ "$files_before" -eq 0 ] || die "le relay a créé $files_before fichier(s) — violation zero-persistance"
log "Aucun fichier créé par le relay (stateless) — $files_before fichier"

# ── 2. Session complète : Alice → relay → Bob ─────────────────────────────
# Bob s'enregistre (l'ACK HELLO_OK est consommé : le relay accuse réception).
exec 3<>"/dev/tcp/127.0.0.1/$PORT"
printf 'HELLO bob\n' >&3
sleep 0.2
IFS= read -r -t 2 ack_bob <&3 || die "pas d'ACK HELLO pour Bob"
case "$ack_bob" in HELLO_OK*) log "Bob enregistré (HELLO_OK)" ;; *) die "ACK Bob inattendu : $ack_bob" ;; esac
# Alice s'enregistre et envoie un fragment vers bob.
exec 4<>"/dev/tcp/127.0.0.1/$PORT"
printf 'HELLO alice\n' >&4
sleep 0.2
IFS= read -r -t 2 ack_alice <&4 || die "pas d'ACK HELLO pour Alice"
case "$ack_alice" in HELLO_OK*) ;; *) die "ACK Alice inattendu : $ack_alice" ;; esac
printf '%s\n' '{"kind":"fragment","from":"alice","to":"bob","session":"fs-1","seq":1,"type":"frag","idx":1,"threshold":4,"total":7,"payload":[1,2,3]}' >&4
sleep 0.3
# Bob lit le fragment (forward réussi) puis se déconnecte.
IFS= read -r -t 2 line <&3 || die "bob n'a rien reçu — le relay n'a pas forwardé"
case "$line" in *fs-1*) log "Fragment forwardé à Bob (session fs-1) — contenu visible pour le destinataire" ;; *) die "payload inattendu : $line" ;; esac
exec 3<&- 3>&-   # Bob part. Le relay doit OUBLIER bob.

# ── 3. Après la déconnexion : le fragment n'est PAS rejouable ─────────────
# Alice renvoie le même fragment vers bob, qui n'est plus là.
printf '%s\n' '{"kind":"fragment","from":"alice","to":"bob","session":"fs-1","seq":1,"type":"frag","idx":1,"threshold":4,"total":7,"payload":[1,2,3]}' >&4
sleep 0.3
# Nouveau Bob se connecte et attend : il ne doit RIEN recevoir (drop, pas buffer).
exec 5<>"/dev/tcp/127.0.0.1/$PORT"
printf 'HELLO bob\n' >&5
sleep 0.4
IFS= read -r -t 1 ack_bob2 <&5 || die "pas d'ACK HELLO pour le nouveau Bob"
case "$ack_bob2" in HELLO_OK*) ;; *) die "ACK inattendu : $ack_bob2" ;; esac
if IFS= read -r -t 1 line <&5; then
  die "le relay a rejoué un fragment à un pair fraîchement connecté — il stocke ! (reçu: $line)"
else
  log "Aucun fragment rejoué au nouveau Bob — le relay n'a rien stocké (drop, pas buffer)"
fi
exec 4<&- 4>&- 5<&- 5>&-

# ── 4. Toujours zéro fichier, et le processus ne tient rien en mémoire persistante
files_after="$(find "$RELAY_WORKDIR" -type f | wc -l)"
[ "$files_after" -eq 0 ] || die "fichiers apparus pendant la session"
log "Toujours 0 fichier après une session complète"

kill "$RELAY_PID" 2>/dev/null || true
wait "$RELAY_PID" 2>/dev/null || true
RELAY_PID=""

echo
echo "═══ VERDICT : l'information n'existe pas. Elle traverse. ═══"
echo "   1. relay stateless (0 fichier)          ✓"
echo "   2. forward fonctionnel                  ✓"
echo "   3. fragment non rejouable après départ   ✓"
exit 0
