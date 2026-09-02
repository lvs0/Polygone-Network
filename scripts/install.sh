#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════
# Polygone — One-Click Installer
# ═══════════════════════════════════════════════════════════════
# curl -fsSL https://polygone.network/install.sh | bash
#
# Détection automatique : Linux/macOS/Windows (WSL), architecture,
# dépendances, installation binaire ou compilation from source.
#
# Basé sur les patterns de : rustup, Homebrew, Deno, Bun.
# ═══════════════════════════════════════════════════════════════
set -euo pipefail

# Couleurs
RED='\033[0;31m'
GREEN='\033[0;32m'
AMBER='\033[0;33m'
NC='\033[0m' # No Color

log_info() { echo -e "${GREEN}▸${NC} $1"; }
log_warn() { echo -e "${AMBER}⚠${NC} $1"; }
log_error() { echo -e "${RED}✗${NC} $1" >&2; }

# Détection OS
detect_os() {
    case "$(uname -s)" in
        Linux*)     echo "linux";;
        Darwin*)    echo "macos";;
        CYGWIN*|MINGW*|MSYS*) echo "windows";;
        *)          echo "unknown";;
    esac
}

# Détection architecture
detect_arch() {
    case "$(uname -m)" in
        x86_64|amd64)   echo "x86_64";;
        aarch64|arm64)  echo "aarch64";;
        armv7l)         echo "armv7";;
        *)              echo "unknown";;
    esac
}

# Vérifier dépendances
check_deps() {
    local missing=()
    for cmd in curl git; do
        if ! command -v "$cmd" &> /dev/null; then
            missing+=("$cmd")
        fi
    done
    if [ ${#missing[@]} -gt 0 ]; then
        log_error "Dépendances manquantes : ${missing[*]}"
        log_info "Installez-les d'abord (ex: sudo apt install curl git)"
        exit 1
    fi
}

# Installation binaire (si release disponible)
install_binary() {
    local os=$1 arch=$2
    local version="2.0.0"
    local url="https://github.com/lvs0/Polygone-Network/releases/download/v${version}/polygone-${os}-${arch}"
    local sha_url="${url}.sha256"

    log_info "Tentative d'installation binaire v${version}..."

    if curl -fsSL "$url" -o /tmp/polygone 2>/dev/null; then
        chmod +x /tmp/polygone
        
        # Verify SHA256 if available
        if curl -fsSL "$sha_url" -o /tmp/polygone.sha256 2>/dev/null; then
            local expected=$(cat /tmp/polygone.sha256 | awk '{print $1}')
            local actual=$(sha256sum /tmp/polygone | awk '{print $1}')
            if [ "$expected" = "$actual" ]; then
                log_info "✓ Signature SHA256 vérifiée"
            else
                log_error "Signature SHA256 invalide"
                rm -f /tmp/polygone /tmp/polygone.sha256
                return 1
            fi
            rm -f /tmp/polygone.sha256
        fi
        
        sudo mv /tmp/polygone /usr/local/bin/polygone
        log_info "✓ Binaire installé : /usr/local/bin/polygone"
        return 0
    else
        log_warn "Binaire non disponible pour ${os}-${arch}, compilation..."
        return 1
    fi
}

# Installation from source
install_source() {
    log_info "Compilation from source..."

    # Vérifier Rust
    if ! command -v cargo &> /dev/null; then
        log_warn "Rust non détecté, installation via rustup..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "$HOME/.cargo/env"
    fi

    # Clone et build
    local tmp_dir=$(mktemp -d)
    git clone --depth 1 https://github.com/lvs0/Polygone-Network.git "$tmp_dir"
    cd "$tmp_dir"

    log_info "Build release (peut prendre 5-10 min)..."
    cargo build --release --workspace

    # Install
    sudo cp target/release/polygone /usr/local/bin/
    sudo cp target/release/polygone-relay /usr/local/bin/ 2>/dev/null || true
    sudo cp target/release/polygoned /usr/local/bin/ 2>/dev/null || true

    log_info "✓ Binaires installés depuis source"
    rm -rf "$tmp_dir"
}

# Configuration initiale
setup_config() {
    local config_dir="$HOME/.config/polygone"
    mkdir -p "$config_dir"

    if [ ! -f "$config_dir/daemon.toml" ]; then
        cat > "$config_dir/daemon.toml" <<'EOF'
tier = "Balanced"
EOF
        log_info "✓ Config créée : $config_dir/daemon.toml"
    fi
}

# Message de bienvenue
welcome() {
    cat <<'EOF'

╔══════════════════════════════════════════════════════════════╗
║                    ⬡ Polygone installé !                     ║
╚══════════════════════════════════════════════════════════════╝

Commandes disponibles :
  polygone              # TUI (Envoyer / Quitter)
  polygone premier-soir # Scénario guidé E2E (5 min)
  polygone verite       # Forensique locale
  polygone --help       # Toutes les commandes

Documentation : https://github.com/lvs0/Polygone-Network
Support : https://payrequest.me/lvs0

« L'information n'existe pas. Elle traverse. »

EOF
}

# Main
main() {
    log_info "Détection du système..."

    local os=$(detect_os)
    local arch=$(detect_arch)

    log_info "OS : $os | Arch : $arch"

    if [ "$os" = "unknown" ] || [ "$arch" = "unknown" ]; then
        log_error "Système non supporté"
        exit 1
    fi

    check_deps

    # Essayer binaire d'abord, fallback sur source
    if ! install_binary "$os" "$arch"; then
        install_source
    fi

    setup_config
    welcome
}

main "$@"
