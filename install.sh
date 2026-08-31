#!/usr/bin/env bash
# Polygone Network — Installateur universel 1-clic
# Usage : curl -fsSL https://polygone.network/install | bash
# Ou :   wget -qO- https://polygone.network/install | bash
#
# Support : Linux (x86_64, aarch64), macOS (x86_64, arm64), Windows (via WSL2)
# Prérequis : aucun (Rust toolchain installé automatiquement via rustup)

set -euo pipefail

# ─── Constantes ──────────────────────────────────────────────────────────────
readonly REPO="lvs0/Polygone-Network"
readonly BINARY_NAME="polygone"
readonly CLIENT_BINARY="polygone-client"
readonly DAEMON_BINARY="polygoned"
readonly INSTALL_DIR="${POLYGONE_INSTALL_DIR:-$HOME/.local/bin}"
readonly CONFIG_DIR="${POLYGONE_CONFIG_DIR:-$HOME/.config/polygone}"
readonly DATA_DIR="${POLYGONE_DATA_DIR:-$HOME/.local/share/polygone}"
readonly RUSTUP_DIR="${RUSTUP_DIR:-$HOME/.rustup}"
readonly CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
readonly MIN_RUST_VERSION="1.82.0"
readonly VERSION_TAG="${POLYGONE_VERSION:-latest}"

# Couleurs
readonly RED='\033[0;31m'
readonly GREEN='\033[0;32m'
readonly YELLOW='\033[1;33m'
readonly BLUE='\033[0;34m'
readonly CYAN='\033[0;36m'
readonly NC='\033[0m' # No Color

# ─── Helpers ─────────────────────────────────────────────────────────────────
log()   { echo -e "${BLUE}[polygone]${NC} $*"; }
ok()    { echo -e "${GREEN}[✓]${NC} $*"; }
warn()  { echo -e "${YELLOW}[!]${NC} $*"; }
err()   { echo -e "${RED}[✗]${NC} $*" >&2; }
die()   { err "$*"; exit 1; }

step()  { echo -e "\n${CYAN}▸${NC} $*"; }

has_cmd() { command -v "$1" >/dev/null 2>&1; }

# ─── Détection OS / Arch ────────────────────────────────────────────────────
detect_platform() {
    local os arch
    case "$(uname -s)" in
        Linux*)     os="linux" ;;
        Darwin*)    os="macos" ;;
        CYGWIN*|MINGW*|MSYS*) os="windows" ;;
        *) die "OS non supporté: $(uname -s)" ;;
    esac

    case "$(uname -m)" in
        x86_64|amd64)   arch="x86_64" ;;
        aarch64|arm64)  arch="aarch64" ;;
        *) die "Architecture non supportée: $(uname -m)" ;;
    esac

    echo "${os}-${arch}"
}

# ─── Installation Rust (via rustup) ─────────────────────────────────────────
install_rust() {
    if has_cmd cargo && has_cmd rustc; then
        local current
        current=$(rustc --version | awk '{print $2}')
        if version_ge "$current" "$MIN_RUST_VERSION"; then
            ok "Rust $current déjà installé (≥ $MIN_RUST_VERSION)"
            return 0
        else
            warn "Rust $current < $MIN_RUST_VERSION — mise à jour nécessaire"
        fi
    fi

    step "Installation de Rust toolchain ($MIN_RUST_VERSION+) via rustup..."

    # rustup non-interactif
    export RUSTUP_INIT_SKIP_PATH_CHECK=1
    export RUSTUP_INIT_SKIP_MSVC_CHECK=1

    if ! has_cmd rustup; then
        curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile minimal
        # shellcheck source=/dev/null
        source "$CARGO_HOME/env"
    else
        rustup update stable
        rustup default stable
    fi

    # Vérification
    # shellcheck source=/dev/null
    source "$CARGO_HOME/env"
    local version
    version=$(rustc --version | awk '{print $2}')
    ok "Rust $version installé"
}

version_ge() {
    # Retourne 0 si $1 >= $2 (versions semver simples)
    local v1=$1 v2=$2
    local IFS=.
    local i
    local -a a1 a2
    read -ra a1 <<< "$v1"
    read -ra a2 <<< "$v2"
    for i in 0 1 2; do
        local n1=${a1[i]:-0}
        local n2=${a2[i]:-0}
        (( n1 > n2 )) && return 0
        (( n1 < n2 )) && return 1
    done
    return 0
}

# ─── Build depuis source (GitHub releases si dispo, sinon build local) ──────
fetch_release() {
    local platform=$1
    local tag=$2
    local asset_name

    # Mapping platform → asset GitHub release
    case "$platform" in
        linux-x86_64)     asset_name="${BINARY_NAME}-${platform}.tar.gz" ;;
        linux-aarch64)    asset_name="${BINARY_NAME}-${platform}.tar.gz" ;;
        macos-x86_64)     asset_name="${BINARY_NAME}-${platform}.tar.gz" ;;
        macos-aarch64)    asset_name="${BINARY_NAME}-${platform}.tar.gz" ;;
        *)                return 1 ;;
    esac

    local url
    if [[ "$tag" == "latest" ]]; then
        url="https://github.com/${REPO}/releases/latest/download/${asset_name}"
    else
        url="https://github.com/${REPO}/releases/download/${tag}/${asset_name}"
    fi

    log "Tentative téléchargement release: $url"
    if curl -fsSL -o "/tmp/${asset_name}" "$url" 2>/dev/null; then
        tar -xzf "/tmp/${asset_name}" -C /tmp/
        ok "Release téléchargée et extraite"
        echo "/tmp"
        return 0
    fi
    warn "Aucune release pré-compilée pour $platform (tag: $tag)"
    return 1
}

build_from_source() {
    step "Compilation depuis les sources (peut prendre 3-8 min)..."

    local tmpdir
    tmpdir=$(mktemp -d)
    trap 'rm -rf "$tmpdir"' EXIT

    log "Clone du repo..."
    git clone --depth 1 --branch "${VERSION_TAG#v}" "https://github.com/${REPO}.git" "$tmpdir/polygone" 2>/dev/null || \
        git clone --depth 1 "https://github.com/${REPO}.git" "$tmpdir/polygone"

    cd "$tmpdir/polygone"

    log "Build release (cargo build --release --all-targets)..."
    cargo build --release --all-targets 2>&1 | tail -20

    # Binaires produits
    local target_dir
    target_dir=$(cargo metadata --format-version=1 2>/dev/null | grep -o '"target_directory":"[^"]*"' | cut -d'"' -f4)
    target_dir=${target_dir:-target}

    echo "$tmpdir/polygone/$target_dir/release"
}

# ─── Installation des binaires ──────────────────────────────────────────────
install_binaries() {
    local src_dir=$1
    step "Installation dans $INSTALL_DIR..."

    mkdir -p "$INSTALL_DIR"
    mkdir -p "$CONFIG_DIR"
    mkdir -p "$DATA_DIR"

    # Copie des binaires
    for bin in "$BINARY_NAME" "$CLIENT_BINARY" "$DAEMON_BINARY"; do
        if [[ -f "$src_dir/$bin" ]]; then
            cp "$src_dir/$bin" "$INSTALL_DIR/"
            ok "Installé: $bin → $INSTALL_DIR/$bin"
        elif [[ -f "$src_dir/$bin.exe" ]]; then
            cp "$src_dir/$bin.exe" "$INSTALL_DIR/$bin"
            ok "Installé: $bin.exe → $INSTALL_DIR/$bin"
        fi
    done

    # Vérification
    for bin in "$BINARY_NAME" "$CLIENT_BINARY" "$DAEMON_BINARY"; do
        if [[ -x "$INSTALL_DIR/$bin" ]]; then
            "$INSTALL_DIR/$bin" --version 2>/dev/null | head -1 | sed 's/^/  /'
        fi
    done
}

# ─── Configuration PATH ─────────────────────────────────────────────────────
setup_path() {
    local shell_rc=""
    case "$(basename "${SHELL:-bash}")" in
        bash)   shell_rc="$HOME/.bashrc" ;;
        zsh)    shell_rc="$HOME/.zshrc" ;;
        fish)   shell_rc="$HOME/.config/fish/config.fish" ;;
        *)      shell_rc="$HOME/.profile" ;;
    esac

    if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
        log "Ajout de $INSTALL_DIR au PATH ($shell_rc)..."
        case "$shell_rc" in
            *fish)
                echo "set -gx PATH \$PATH $INSTALL_DIR" >> "$shell_rc"
                ;;
            *)
                echo "export PATH=\"\$PATH:$INSTALL_DIR\"" >> "$shell_rc"
                ;;
        esac
        ok "PATH mis à jour — relancez votre shell ou: source $shell_rc"
    else
        ok "PATH déjà configuré"
    fi
}

# ─── Génération config par défaut ───────────────────────────────────────────
generate_config() {
    if [[ ! -f "$CONFIG_DIR/daemon.toml" ]]; then
        step "Génération config par défaut..."
        "$INSTALL_DIR/$DAEMON_BINARY" --gen-config 2>/dev/null || true
        ok "Config créée: $CONFIG_DIR/daemon.toml"
    else
        ok "Config existante conservée: $CONFIG_DIR/daemon.toml"
    fi
}

# ─── Vérification post-install ──────────────────────────────────────────────
verify_install() {
    step "Vérification..."

    local all_ok=1
    for bin in "$BINARY_NAME" "$CLIENT_BINARY" "$DAEMON_BINARY"; do
        if "$INSTALL_DIR/$bin" --version >/dev/null 2>&1; then
            ok "$bin: OK"
        else
            err "$bin: ÉCHEC"
            all_ok=0
        fi
    done

    if [[ $all_ok -eq 1 ]]; then
        echo
        echo -e "${GREEN}╔══════════════════════════════════════════════════════════╗${NC}"
        echo -e "${GREEN}║  Polygone Network installé avec succès ! 🎉              ║${NC}"
        echo -e "${GREEN}╚══════════════════════════════════════════════════════════╝${NC}"
        echo
        echo "Commandes disponibles :"
        echo "  polygone       — CLI principal (aide: polygone --help)"
        echo "  polygone-client — Interface TUI interactive"
        echo "  polygoned      — Daemon arrière-plan"
        echo
        echo "Prochaines étapes :"
        echo "  1. source ~/.bashrc   (ou relancez votre terminal)"
        echo "  2. polygone-client    (lance le TUI — 2 onglets: Envoyer / Quitter)"
        echo "  3. polygoned start    (démarre le daemon en arrière-plan)"
        echo
        echo "Config: $CONFIG_DIR/daemon.toml"
        echo "Données: $DATA_DIR"
        echo "Logs: journalctl --user -u polygoned -f"
        echo
    else
        die "Installation incomplète — voir erreurs ci-dessus"
    fi
}

# ─── Main ───────────────────────────────────────────────────────────────────
main() {
    echo -e "${CYAN}"
    echo "  ⬡  Polygone Network — Installateur universel"
    echo "  ═══════════════════════════════════════════"
    echo -e "${NC}"

    local platform
    platform=$(detect_platform)
    log "Plateforme détectée: $platform"

    # 1. Rust
    install_rust

    # 2. Essayer release pré-compilée, sinon build
    local bin_dir
    if bin_dir=$(fetch_release "$platform" "$VERSION_TAG"); then
        :
    else
        bin_dir=$(build_from_source)
    fi

    # 3. Installer
    install_binaries "$bin_dir"

    # 4. PATH
    setup_path

    # 5. Config
    generate_config

    # 6. Vérifier
    verify_install
}

# ─── Entrée ─────────────────────────────────────────────────────────────────
main "$@"