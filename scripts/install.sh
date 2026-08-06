#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════
#  POLYGONE — installateur autonome (SPEC §5 "Genius")
#
#  Usage:  curl -fsSL https://polygone.network/install | bash
#  (ou:    bash scripts/install.sh   depuis le repo)
#
#  Ce que ça fait :
#    1. Détecte OS + architecture
#    2. Cherche un binaire précompilé (GitHub release) puis fallback build cargo
#    3. Installe `polygone`, `polygone-relay`, `polygone-client`, `polygoned`
#       dans ~/.local/bin (ou /usr/local/bin si root)
#    4. Génère votre identité au premier lancement (aucune inscription)
#
#  « On voit rien. Et c'est comme ça que ça devrait être. »
# ═══════════════════════════════════════════════════════════════════════════
set -euo pipefail

# ── Couleurs (best-effort) ────────────────────────────────────────────────────
if [ -t 1 ]; then
  BOLD=$'\033[1m'; AMBER=$'\033[33m'; GREEN=$'\033[32m'; CYAN=$'\033[36m'; RED=$'\033[31m'; RESET=$'\033[0m'
else
  BOLD=""; AMBER=""; GREEN=""; CYAN=""; RED=""; RESET=""
fi

log()  { printf '%s\n' "${GREEN}✓${RESET} $*"; }
info() { printf '%s\n' "${CYAN}›${RESET} $*"; }
warn() { printf '%s\n' "${AMBER}⚠${RESET} $*"; }
die()  { printf '%s\n' "${RED}✖${RESET} $*" >&2; exit 1; }

VERSION="v2.0.0-rc2"
REPO="lvs0/Polygone-Network"
INSTALL_DIR="${POLYGONE_INSTALL_DIR:-}"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

# ── Bannière ──────────────────────────────────────────────────────────────────
printf '%s\n' \
  "${BOLD}${AMBER}╔══════════════════════════════════════════════════╗${RESET}" \
  "${BOLD}${AMBER}║      ⬡  P O L Y G O N E   $VERSION        ║${RESET}" \
  "${BOLD}${AMBER}║   L'information n'existe pas. Elle traverse.    ║${RESET}" \
  "${BOLD}${AMBER}╚══════════════════════════════════════════════════╝${RESET}"

# ── 1. Détection plateforme ──────────────────────────────────────────────────
OS="$(uname -s)"
ARCH="$(uname -m)"
case "$OS" in
  Linux)  TARGET_OS="linux" ;;
  Darwin) TARGET_OS="macos" ;;
  *)      die "OS non supporté : $OS (Linux/macOS seulement pour l'instant)" ;;
esac
case "$ARCH" in
  x86_64|amd64) TARGET_ARCH="x86_64" ;;
  aarch64|arm64) TARGET_ARCH="aarch64" ;;
  *) die "Architecture non supportée : $ARCH" ;;
esac
info "Plateforme : $TARGET_OS/$TARGET_ARCH"

# ── 2. Répertoire d'installation ──────────────────────────────────────────────
if [ -z "$INSTALL_DIR" ]; then
  if [ "$(id -u)" = "0" ]; then
    INSTALL_DIR="/usr/local/bin"
  else
    INSTALL_DIR="$HOME/.local/bin"
  fi
fi
mkdir -p "$INSTALL_DIR"
info "Installation dans : $INSTALL_DIR"

# ── 3. Binaire ────────────────────────────────────────────────────────────────
install_prebuilt() {
  local url="https://github.com/$REPO/releases/download/$VERSION/polygone-$TARGET_OS-$TARGET_ARCH.tar.gz"
  info "Téléchargement du binaire précompilé…"
  if curl -fsSL --max-time 60 "$url" -o "$TMP_DIR/polygone.tar.gz" 2>/dev/null; then
    tar -xzf "$TMP_DIR/polygone.tar.gz" -C "$TMP_DIR"
    return 0
  fi
  return 1
}

install_from_source() {
  local repo_dir="$TMP_DIR/polygone-src"
  local local_repo
  local_repo="$(cd "$(dirname "$0")/.." 2>/dev/null && pwd)" || true

  info "Pas de release précompilée — build depuis les sources (nécessite cargo)…"
  command -v cargo >/dev/null 2>&1 || die \
    "cargo introuvable. Installez Rust (https://rustup.rs) puis relancez, ou attendez une release."

  local build_dir
  if [ -n "$local_repo" ] && [ -f "$local_repo/Cargo.toml" ]; then
    build_dir="$local_repo"          # on est DANS le repo : on build le code local
    info "Build depuis le repo local : $build_dir"
  else
    build_dir="$repo_dir"
    git clone --depth 1 "https://github.com/$REPO" "$build_dir" \
      || die "Impossible de récupérer les sources."
  fi

  ( cd "$build_dir" && cargo build --release -p polygone-client -p polygone-relay -p polygoned )

  mkdir -p "$TMP_DIR/bin"
  local missing=0 b
  for b in polygone polygone-client polygone-relay polygoned; do
    if [ -f "$build_dir/target/release/$b" ]; then
      cp "$build_dir/target/release/$b" "$TMP_DIR/bin/"
    else
      warn "binaire '$b' absent du build (dépôt à jour ?)"
      missing=1
    fi
  done
  [ "$missing" = "0" ] \
    || die "Binaires incomplets — le dépôt source est en retard sur le code attendu. Poussez le dernier commit puis relancez."
}

# ── 3. Binaire ────────────────────────────────────────────────────────────────
BIN="polygone"
TARGET_BIN="$INSTALL_DIR/$BIN"

if install_prebuilt; then
  mkdir -p "$TMP_DIR/bin"
  cp "$TMP_DIR/polygone" "$TMP_DIR/polygone-relay" "$TMP_DIR/polygone-client" \
     "$TMP_DIR/polygoned" "$TMP_DIR/bin/" 2>/dev/null \
    || { warn "release incomplète — build source à la place"; install_from_source; }
else
  install_from_source
fi

cp "$TMP_DIR/bin/polygone" "$TMP_DIR/bin/polygone-relay" "$TMP_DIR/bin/polygone-client" \
   "$TMP_DIR/bin/polygoned" "$INSTALL_DIR/" 2>/dev/null \
  || die "Échec de copie des binaires vers $INSTALL_DIR"

chmod +x "$INSTALL_DIR/polygone" "$INSTALL_DIR/polygone-relay" "$INSTALL_DIR/polygone-client" "$INSTALL_DIR/polygoned" 2>/dev/null || true
log "Binaires installés : $INSTALL_DIR"

# ── 4. PATH ───────────────────────────────────────────────────────────────────
case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *)
    warn "$INSTALL_DIR n'est pas dans votre PATH."
    info "Ajoutez cette ligne à votre ~/.bashrc (ou ~/.zshrc) :"
    printf '    %s\n' "export PATH=\"\$HOME/.local/bin:\$PATH\""
    ;;
esac

# ── 5. Premier lancement ─────────────────────────────────────────────────────
printf '\n%s\n' "${BOLD}${GREEN}Polygone est prêt.${RESET}"
printf '%s\n' \
  "" \
  "  ${BOLD}polygone${RESET}              → la TUI (:envoyer / :quitter)" \
  "  ${BOLD}polygone demo${RESET}         → démo E2E post-quantique (60 s)" \
  "  ${BOLD}polygone clef${RESET}         → votre clef publique (à partager)" \
  "  ${BOLD}polygone envoyer${RESET}      → chiffrer + fragmenter un message" \
  "  ${BOLD}polygone recevoir${RESET}     → reconstruire + déchiffrer (4/7)" \
  "" \
  "  Identité générée au premier lancement — aucune inscription, aucune télémétrie." \
  "  L'information n'existe pas. Elle traverse."

exit 0
