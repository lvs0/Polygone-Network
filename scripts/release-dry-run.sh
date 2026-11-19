#!/usr/bin/env bash
# release-dry-run.sh — release en essai à blanc, au format attendu par install.sh.
#
# Ne publie rien, ne tague rien, ne pousse rien. Il :
#   1. lit la version dans Cargo.toml et l'hôte de compilation ;
#   2. rassemble les quatre binaires (déjà construits, ou --build explicite) ;
#   3. vérifie que le contrat de `install.sh` n'a pas dérivé ;
#   4. fabrique `polygone-<os>-<arch>.tar.gz` (mtime et propriétaires figés) ;
#   5. écrit `SHA256SUMS` au format `<hash>  <asset>` ;
#   6. applique les règles d'acceptation de l'installeur à l'archive produite :
#      empreinte, ensemble de membres, types de fichiers, extraction ;
#   7. prouve qu'une altération est détectée ;
#   8. s'arrête. Aucun téléversement.
#
# Ce que ce script ne prouve pas : que la release GitHub sera publiée, ni que
# `install.sh` exécuté depuis Internet trouvera le tag. Il prouve que
# l'artefact produit est celui que l'installeur accepte, aujourd'hui.
#
# Usage :
#   ./scripts/release-dry-run.sh                  # binaires existants
#   ./scripts/release-dry-run.sh --build          # compile d'abord (lourd)
#   POLYGONE_BIN_DIR=... POLYGONE_DIST_DIR=... ./scripts/release-dry-run.sh
set -Eeuo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="${POLYGONE_BIN_DIR:-$ROOT/target/release}"
OUT_DIR="${POLYGONE_DIST_DIR:-$ROOT/dist}"
BUILD=0
[[ "${1:-}" == "--build" ]] && BUILD=1

# Contrat de install.sh (voir `fetch_release` et la validation des membres).
BINARIES=(polygone polygone-relay polygoned polygone-gateway)
WEB_REQUIRED=(web/index.html web/manifest.webmanifest web/sw.js)

step() { printf '\n== %s ==\n' "$*"; }
fail() { printf '\n✖ FAIL: %s\n' "$*" >&2; exit 1; }

step "0. garde-fous"
[[ -n "$OUT_DIR" && "$OUT_DIR" != "/" ]] || fail "POLYGONE_DIST_DIR invalide : « $OUT_DIR »"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"
[[ -n "$VERSION" ]] || fail "version illisible dans $ROOT/Cargo.toml"
HOST="$(rustc -vV | sed -n 's/^host: //p')"
case "$HOST" in
  x86_64-unknown-linux-gnu)   PLATFORM="linux-x86_64" ;;
  aarch64-unknown-linux-gnu)  PLATFORM="linux-aarch64" ;;
  x86_64-apple-darwin)        PLATFORM="macos-x86_64" ;;
  aarch64-apple-darwin)       PLATFORM="macos-aarch64" ;;
  *) fail "hôte non supporté par install.sh : « $HOST » (attendu linux/macos x86_64/aarch64)" ;;
esac
ASSET="polygone-$PLATFORM.tar.gz"
STAGE="$OUT_DIR/polygone-$PLATFORM"
printf 'version %s · hôte %s · plateforme %s\n' "$VERSION" "$HOST" "$PLATFORM"

if [[ "$BUILD" -eq 1 ]]; then
  step "1. compilation des binaires de release (lourd, explicite)"
  cargo build --release --workspace --manifest-path "$ROOT/Cargo.toml"
else
  step "1. réutilisation des binaires existants (aucune compilation)"
  printf 'répertoire : %s\n' "$BIN_DIR"
fi

step "2. contrat de install.sh"
INSTALLER="$ROOT/install.sh"
[[ -f "$INSTALLER" ]] || fail "install.sh introuvable : le contrat de release ne peut pas être vérifié"
grep -qF 'asset_name="${BINARY_NAME}-${platform}.tar.gz"' "$INSTALLER" \
  || fail "install.sh ne nomme plus l'artefact « \${BINARY_NAME}-\${platform}.tar.gz » : contrat rompu"
grep -qF 'base_url="https://github.com/${REPO}/releases/latest/download"' "$INSTALLER" \
  || fail "install.sh ne télécharge plus depuis releases/latest/download : contrat rompu"
grep -qF 'sums_url="$base_url/SHA256SUMS"' "$INSTALLER" \
  || fail "install.sh ne lit plus SHA256SUMS à la racine de la release : contrat rompu"
for bin in "${BINARIES[@]}"; do
  grep -qF -- "-:$bin" "$INSTALLER" || fail "install.sh n'accepte plus le membre racine « $bin »"
done
for member in "${WEB_REQUIRED[@]}"; do
  grep -qF -- "$member" "$INSTALLER" || fail "install.sh n'exige plus « $member »"
done
[[ -d "$ROOT/web" ]] || fail "répertoire web/ absent : l'archive serait refusée"
printf 'contrat vérifié : %s binaires + web/ + SHA256SUMS\n' "${#BINARIES[@]}"

step "3. rassemblement"
[[ -d "$BIN_DIR" ]] || fail "répertoire de binaires absent : $BIN_DIR"
rm -rf -- "$STAGE"
mkdir -p -- "$STAGE"
for bin in "${BINARIES[@]}"; do
  [[ -f "$BIN_DIR/$bin" ]] || fail "binaire requis absent : $BIN_DIR/$bin (les quatre sont obligatoires dans l'archive)"
  cp -- "$BIN_DIR/$bin" "$STAGE/$bin"
  chmod +x "$STAGE/$bin"
  printf '  + %-18s %s octets\n' "$bin" "$(wc -c <"$STAGE/$bin")"
done
cp -R -- "$ROOT/web" "$STAGE/web"
WEB_COUNT="$(find "$STAGE/web" -type f | wc -l)"
[[ "$WEB_COUNT" -gt 0 ]] || fail "web/ copié vide"
printf '  + %-18s %s fichiers\n' "web/" "$WEB_COUNT"

step "4. archive reproductible"
# mtime, propriétaires et ordre figés : deux exécutions sur le même contenu
# produisent la même archive, donc la même empreinte.
tar --sort=name --mtime='@1700000000' --owner=0 --group=0 --numeric-owner \
    -czf "$OUT_DIR/$ASSET" -C "$STAGE" "${BINARIES[@]}" web
printf '%s (%s octets)\n' "$ASSET" "$(wc -c <"$OUT_DIR/$ASSET")"

step "5. manifeste"
( cd -- "$OUT_DIR" && sha256sum -- "$ASSET" ) >"$OUT_DIR/SHA256SUMS"
cat "$OUT_DIR/SHA256SUMS"

step "6. règles d'acceptation de l'installeur"
EXPECTED="$(awk -v asset="$ASSET" '$2 == asset { print $1 }' "$OUT_DIR/SHA256SUMS")"
[[ "$EXPECTED" =~ ^[[:xdigit:]]{64}$ ]] || fail "empreinte absente, ambiguë ou mal formée pour $ASSET"
ACTUAL="$(sha256sum -- "$OUT_DIR/$ASSET" | awk '{print $1}')"
[[ "${EXPECTED,,}" == "${ACTUAL,,}" ]] || fail "SHA256 de l'archive incohérent"

LIST="$(mktemp)"; VERBOSE="$(mktemp)"
cleanup() { set +e; rm -f -- "$LIST" "$VERBOSE"; rm -rf -- "$EXTRACT"; }
trap cleanup EXIT INT TERM
tar -tzf "$OUT_DIR/$ASSET" >"$LIST" || fail "archive illisible"
tar -tvzf "$OUT_DIR/$ASSET" >"$VERBOSE" || fail "types de l'archive illisibles"

VALID=1; HAS_WEB_DIR=0; HAS_INDEX=0; HAS_MANIFEST=0; HAS_WORKER=0
while IFS= read -r member; do
  case "$member" in
    polygone|polygone-relay|polygoned|polygone-gateway) ;;
    web/) HAS_WEB_DIR=1 ;;
    web/*)
      [[ "${member#web/}" =~ ^[A-Za-z0-9_-]+([.][A-Za-z0-9_-]+)*([/][A-Za-z0-9_-]+([.][A-Za-z0-9_-]+)*)*$ ]] || VALID=0
      [[ "$member" == "web/index.html" ]] && HAS_INDEX=1
      [[ "$member" == "web/manifest.webmanifest" ]] && HAS_MANIFEST=1
      [[ "$member" == "web/sw.js" ]] && HAS_WORKER=1
      ;;
    *) VALID=0 ;;
  esac
done <"$LIST"
MEMBER_COUNT="$(wc -l <"$LIST")"
[[ "$MEMBER_COUNT" -ge 8 ]] || VALID=0
[[ "$HAS_WEB_DIR$HAS_INDEX$HAS_MANIFEST$HAS_WORKER" == "1111" ]] || VALID=0

while IFS= read -r entry; do
  type="${entry:0:1}"
  name="${entry##* }"; name="${name#./}"
  case "$type:$name" in
    -:polygone|-:polygone-relay|-:polygoned|-:polygone-gateway) ;;
    d:web/) ;;
    -:web/*) ;;
    *) VALID=0 ;;
  esac
done <"$VERBOSE"
[[ "$VALID" -eq 1 ]] || fail "archive refusée par les règles de install.sh (membre, type ou ressource manquante)"

EXTRACT="$(mktemp -d)"
tar -xzf "$OUT_DIR/$ASSET" -C "$EXTRACT" --no-same-owner --no-same-permissions || fail "extraction échouée"
for bin in "${BINARIES[@]}"; do
  [[ -f "$EXTRACT/$bin" && ! -L "$EXTRACT/$bin" ]] || fail "après extraction, « $bin » manquant ou non régulier"
done
printf 'archive acceptée : %s membres, 4 binaires réguliers, web/ complet\n' "$MEMBER_COUNT"

step "7. preuve de détection d'altération"
TMP="$(mktemp -d)"
cp -- "$OUT_DIR/$ASSET" "$TMP/$ASSET"
cp -- "$OUT_DIR/SHA256SUMS" "$TMP/SHA256SUMS"
printf 'x' >>"$TMP/$ASSET"
TAMPERED="$(sha256sum -- "$TMP/$ASSET" | awk '{print $1}')"
if [[ "${TAMPERED,,}" == "${EXPECTED,,}" ]]; then
  fail "archive altérée mais empreinte identique : le manifeste ne prouve rien"
fi
printf 'altération détectée (empreinte différente, attendu)\n'
rm -rf -- "$TMP"

step "DRY RUN"
printf 'artefact : %s\n' "$OUT_DIR/$ASSET"
printf 'manifeste : %s\n' "$OUT_DIR/SHA256SUMS"
printf 'Aucune publication, aucun tag, aucun push. Rien n a été téléversé.\n'
