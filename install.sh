#!/usr/bin/env bash
# Polygone Network — Installateur universel 1-clic
# Usage : curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/lvs0/Polygone-Network/main/install.sh | bash
#
# Support : Linux (x86_64, aarch64), macOS (x86_64, arm64), Windows (via WSL2)
# Prérequis : aucun (Rust toolchain installé automatiquement via rustup)

set -euo pipefail

# ─── Constantes ──────────────────────────────────────────────────────────────
readonly REPO="lvs0/Polygone-Network"
readonly BINARY_NAME="polygone"
readonly CLIENT_BINARY="polygone-client"
readonly RELAY_BINARY="polygone-relay"
readonly DAEMON_BINARY="polygoned"
readonly GATEWAY_BINARY="polygone-gateway"
readonly INSTALL_DIR="${POLYGONE_INSTALL_DIR:-$HOME/.local/bin}"
readonly CONFIG_DIR="${POLYGONE_CONFIG_DIR:-$HOME/.config/polygone}"
readonly DATA_DIR="${POLYGONE_DATA_DIR:-$HOME/.local/share/polygone}"
readonly SHARE_DIR="${POLYGONE_SHARE_DIR:-$(dirname "$INSTALL_DIR")/share/polygone}"
readonly RUSTUP_DIR="${RUSTUP_DIR:-$HOME/.rustup}"
readonly CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
readonly MIN_RUST_VERSION="1.85.0"
readonly VERSION_TAG="${POLYGONE_VERSION:-latest}"

# Couleurs
readonly RED='\033[0;31m'
readonly GREEN='\033[0;32m'
readonly YELLOW='\033[1;33m'
readonly BLUE='\033[0;34m'
readonly CYAN='\033[0;36m'
readonly NC='\033[0m' # No Color

# ─── Helpers ─────────────────────────────────────────────────────────────────
log()   { echo -e "${BLUE}[polygone]${NC} $*" >&2; }
ok()    { echo -e "${GREEN}[✓]${NC} $*" >&2; }
warn()  { echo -e "${YELLOW}[!]${NC} $*" >&2; }
err()   { echo -e "${RED}[✗]${NC} $*" >&2; }
die()   { err "$*"; exit 1; }

step()  { echo -e "\n${CYAN}▸${NC} $*" >&2; }

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

    case "$platform" in
        linux-x86_64|linux-aarch64|macos-x86_64|macos-aarch64)
            asset_name="${BINARY_NAME}-${platform}.tar.gz"
            ;;
        *) return 1 ;;
    esac

    local base_url
    if [[ "$tag" != "latest" && "$tag" != v* ]]; then
        tag="v$tag"
    fi
    if [[ "$tag" == "latest" ]]; then
        base_url="https://github.com/${REPO}/releases/latest/download"
    else
        base_url="https://github.com/${REPO}/releases/download/${tag}"
    fi

    local tmp_dir archive sums_file extract_dir sums_url
    tmp_dir=$(mktemp -d "${TMPDIR:-/tmp}/polygone-release.XXXXXX") || die "Impossible de créer le dossier temporaire"
    archive="$tmp_dir/$asset_name"
    sums_file="$tmp_dir/SHA256SUMS"
    extract_dir="$tmp_dir/extract"
    sums_url="$base_url/SHA256SUMS"

    log "Téléchargement de la release $asset_name"
    if ! curl --proto '=https' --tlsv1.2 --proto-redir '=https' -fsSL \
        -o "$archive" "$base_url/$asset_name"; then
        warn "Aucune release pré-compilée pour $platform (tag: $tag)"
        rm -rf -- "$tmp_dir"
        return 1
    fi

    # Fail closed: a release archive without its published checksum is never
    # installed. The caller falls back to a source build instead.
    if ! curl --proto '=https' --tlsv1.2 --proto-redir '=https' -fsSL \
        -o "$sums_file" "$sums_url"; then
        err "SHA256SUMS absent pour $asset_name — release refusée (fail-closed)."
        rm -rf -- "$tmp_dir"
        return 1
    fi

    local expected actual
    expected=$(awk -v asset="$asset_name" '$2 == asset { print $1 }' "$sums_file")
    if [[ ! "$expected" =~ ^[[:xdigit:]]{64}$ ]]; then
        err "Empreinte SHA256 absente, ambiguë ou mal formée pour $asset_name."
        rm -rf -- "$tmp_dir"
        return 1
    fi
    if has_cmd sha256sum; then
        actual=$(sha256sum "$archive" | awk '{print $1}')
    elif has_cmd shasum; then
        actual=$(shasum -a 256 "$archive" | awk '{print $1}')
    else
        err "sha256sum/shasum absent — impossible de vérifier la release (fail-closed)."
        rm -rf -- "$tmp_dir"
        return 1
    fi
    local expected_lower actual_lower
    expected_lower=$(printf '%s' "$expected" | tr '[:upper:]' '[:lower:]')
    actual_lower=$(printf '%s' "$actual" | tr '[:upper:]' '[:lower:]')
    if [[ "$expected_lower" != "$actual_lower" ]]; then
        err "SHA256 invalide pour $asset_name (attendu $expected, obtenu $actual)."
        rm -rf -- "$tmp_dir"
        return 1
    fi
    ok "SHA256 vérifié pour $asset_name"

    # Release layout: four root-level binaries plus regular files under web/.
    if ! tar -tzf "$archive" > "$tmp_dir/archive.list"; then
        err "Archive tar.gz illisible."
        rm -rf -- "$tmp_dir"
        return 1
    fi
    if ! tar -tvzf "$archive" > "$tmp_dir/archive.verbose"; then
        err "Impossible de vérifier les types de fichiers de l'archive."
        rm -rf -- "$tmp_dir"
        return 1
    fi
    local member verbose_entry relative member_count=0 valid=1
    local has_client=0 has_relay=0 has_daemon=0 has_gateway=0
    local has_web_dir=0 has_web_index=0 has_web_manifest=0 has_web_worker=0
    : > "$tmp_dir/archive.seen"
    while IFS= read -r member; do
        member="${member#./}"
        member_count=$((member_count + 1))
        if grep -Fxq -- "$member" "$tmp_dir/archive.seen"; then
            valid=0
        fi
        printf '%s\n' "$member" >> "$tmp_dir/archive.seen"
        case "$member" in
            polygone) has_client=1 ;;
            polygone-relay) has_relay=1 ;;
            polygoned) has_daemon=1 ;;
            polygone-gateway) has_gateway=1 ;;
            web/) has_web_dir=1 ;;
            web/*)
                relative="${member#web/}"
                if [[ ! "$relative" =~ ^[A-Za-z0-9_-]+([.][A-Za-z0-9_-]+)*([/][A-Za-z0-9_-]+([.][A-Za-z0-9_-]+)*)*$ ]]; then
                    valid=0
                fi
                [[ "$member" == "web/index.html" ]] && has_web_index=1
                [[ "$member" == "web/manifest.webmanifest" ]] && has_web_manifest=1
                [[ "$member" == "web/sw.js" ]] && has_web_worker=1
                ;;
            *) valid=0 ;;
        esac
    done < "$tmp_dir/archive.list"
    if [[ $member_count -lt 8 || $has_client -ne 1 || $has_relay -ne 1 || $has_daemon -ne 1 || $has_gateway -ne 1 || $has_web_dir -ne 1 || $has_web_index -ne 1 || $has_web_manifest -ne 1 || $has_web_worker -ne 1 ]]; then
        valid=0
    fi

    # Reject symlinks, hardlinks, devices, FIFOs and every file type except
    # regular files/directories. Directories are only allowed under web/.
    while IFS= read -r verbose_entry; do
        local file_type="${verbose_entry:0:1}"
        local member_name="${verbose_entry##* }"
        member_name="${member_name#./}"
        case "$file_type:$member_name" in
            -:polygone|-:polygone-relay|-:polygoned|-:polygone-gateway) ;;
            d:web/) ;;
            -:web/*) ;;
            *) valid=0 ;;
        esac
    done < "$tmp_dir/archive.verbose"

    if [[ $valid -ne 1 ]]; then
        err "L'archive contient des chemins inattendus, des liens ou un ensemble de ressources incomplet."
        rm -rf -- "$tmp_dir"
        return 1
    fi

    mkdir -p "$extract_dir"
    if ! tar -xzf "$archive" -C "$extract_dir" --no-same-owner --no-same-permissions; then
        err "Extraction échouée pour $asset_name."
        rm -rf -- "$tmp_dir"
        return 1
    fi
    for member in polygone polygone-relay polygoned polygone-gateway; do
        if [[ ! -f "$extract_dir/$member" || -L "$extract_dir/$member" ]]; then
            err "Binaire manquant ou non régulier après extraction : $member"
            rm -rf -- "$tmp_dir"
            return 1
        fi
    done
    for member in web/index.html web/manifest.webmanifest web/sw.js; do
        if [[ ! -f "$extract_dir/$member" || -L "$extract_dir/$member" ]]; then
            err "Ressource web manquante ou non régulière après extraction : $member"
            rm -rf -- "$tmp_dir"
            return 1
        fi
    done

    ok "Release téléchargée, vérifiée et extraite"
    echo "$extract_dir"
}

build_from_source() {
    step "Compilation depuis les sources..."

    local tmpdir branch target_dir release_dir
    tmpdir=$(mktemp -d "${TMPDIR:-/tmp}/polygone-build.XXXXXX") || die "Impossible de créer le dossier de build"
    log "Clone du dépôt..."

    if [[ "$VERSION_TAG" == "latest" ]]; then
        git clone --depth 1 "https://github.com/${REPO}.git" "$tmpdir/polygone" >&2
    else
        branch="$VERSION_TAG"
        [[ "$branch" == v* ]] || branch="v$branch"
        git clone --depth 1 --branch "$branch" "https://github.com/${REPO}.git" "$tmpdir/polygone" >&2 || {
            err "Tag source introuvable : $branch"
            rm -rf -- "$tmpdir"
            return 1
        }
    fi

    cd "$tmpdir/polygone"
    log "Build des quatre binaires release..."
    cargo build --release -p polygone-client -p polygone-relay -p polygoned -p polygone-gateway 2>&1 | tail -20 >&2

    target_dir=$(cargo metadata --format-version=1 2>/dev/null | grep -o '"target_directory":"[^"]*"' | cut -d'"' -f4)
    target_dir=${target_dir:-"$tmpdir/polygone/target"}
    release_dir="$target_dir/release"
    for bin in polygone polygone-relay polygoned polygone-gateway; do
        if [[ ! -x "$release_dir/$bin" ]]; then
            err "Binaire compilé manquant : $release_dir/$bin"
            rm -rf -- "$tmpdir"
            return 1
        fi
    done

    if [[ ! -f "$tmpdir/polygone/web/index.html" ]]; then
        err "Ressources web manquantes dans le dépôt cloné."
        rm -rf -- "$tmpdir"
        return 1
    fi
    mkdir -p "$release_dir/web"
    cp -a "$tmpdir/polygone/web/." "$release_dir/web/"

    # The caller copies binaries and web resources, then install_binaries removes this temp tree.
    echo "$release_dir"
}

resolve_binary_dir() {
    local platform=$1
    local bin_dir
    if bin_dir=$(fetch_release "$platform" "$VERSION_TAG"); then
        printf '%s\n' "$bin_dir"
        return 0
    fi

    # Prebuilt releases need no toolchain. Install Rust only for source fallback.
    install_rust >&2
    bin_dir=$(build_from_source)
    printf '%s\n' "$bin_dir"
}

# ─── Installation des binaires ──────────────────────────────────────────────
install_binaries() {
    local src_dir=$1
    step "Installation dans $INSTALL_DIR..."
    [[ -d "$src_dir" ]] || die "Dossier source introuvable : $src_dir"

    for bin in "$BINARY_NAME" "$RELAY_BINARY" "$DAEMON_BINARY" "$GATEWAY_BINARY"; do
        [[ -f "$src_dir/$bin" && ! -L "$src_dir/$bin" ]] || die "Binaire requis absent ou invalide : $src_dir/$bin"
    done
    [[ -f "$src_dir/web/index.html" && ! -L "$src_dir/web/index.html" ]] || die "Ressources web absentes : $src_dir/web/index.html"
    [[ -f "$src_dir/web/manifest.webmanifest" && ! -L "$src_dir/web/manifest.webmanifest" ]] || die "Manifeste PWA absent."
    [[ -f "$src_dir/web/sw.js" && ! -L "$src_dir/web/sw.js" ]] || die "Service worker absent."

    mkdir -p "$INSTALL_DIR" "$CONFIG_DIR" "$DATA_DIR" "$SHARE_DIR/web"
    for bin in "$BINARY_NAME" "$RELAY_BINARY" "$DAEMON_BINARY" "$GATEWAY_BINARY"; do
        install -m 0755 "$src_dir/$bin" "$INSTALL_DIR/$bin"
        ok "Installé : $bin → $INSTALL_DIR/$bin"
    done
    cp -a "$src_dir/web/." "$SHARE_DIR/web/"
    ok "Site/PWA installé : $SHARE_DIR/web"

    # The package exposes one unified binary; this is a compatibility alias,
    # not a second executable (Cargo has no `polygone-client` bin target).
    if [[ -L "$INSTALL_DIR/$CLIENT_BINARY" && ! -e "$INSTALL_DIR/$CLIENT_BINARY" ]]; then
        rm -f -- "$INSTALL_DIR/$CLIENT_BINARY"
    fi
    if [[ ! -e "$INSTALL_DIR/$CLIENT_BINARY" ]]; then
        ln -s "$BINARY_NAME" "$INSTALL_DIR/$CLIENT_BINARY"
        ok "Alias créé : $CLIENT_BINARY → $BINARY_NAME"
    fi

    for bin in "$BINARY_NAME" "$CLIENT_BINARY" "$RELAY_BINARY" "$DAEMON_BINARY" "$GATEWAY_BINARY"; do
        if [[ -x "$INSTALL_DIR/$bin" ]]; then
            "$INSTALL_DIR/$bin" --version 2>/dev/null | head -1 | sed 's/^/  /'
        fi
    done

    # Release and source-build directories are private temporary trees.
    case "$src_dir" in
        "${TMPDIR:-/tmp}"/polygone-release.*/extract)
            rm -rf -- "${src_dir%/extract}"
            ;;
        "${TMPDIR:-/tmp}"/polygone-build.*/polygone/*/release)
            rm -rf -- "${src_dir%%/polygone/*}"
            ;;
    esac
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
        if ! "$INSTALL_DIR/$DAEMON_BINARY" --config "$CONFIG_DIR/daemon.toml" --gen-config; then
            die "Échec de génération de la configuration daemon."
        fi
        [[ -f "$CONFIG_DIR/daemon.toml" ]] || die "Le daemon n'a pas créé $CONFIG_DIR/daemon.toml"
        ok "Config créée: $CONFIG_DIR/daemon.toml"
    else
        ok "Config existante conservée: $CONFIG_DIR/daemon.toml"
    fi
}

# ─── Vérification post-install ──────────────────────────────────────────────
verify_install() {
    step "Vérification..."

    local all_ok=1
    for bin in "$BINARY_NAME" "$CLIENT_BINARY" "$RELAY_BINARY" "$DAEMON_BINARY" "$GATEWAY_BINARY"; do
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
        echo "  polygone       — CLI unifiée + TUI (aide: polygone --help)"
        echo "  polygone-client — alias de compatibilité vers polygone"
        echo "  polygone-relay — relais réseau"
        echo "  polygoned      — daemon arrière-plan"
        echo "  polygone-gateway — site + API locale (loopback uniquement)"
        echo
        echo "Prochaines étapes :"
        echo "  1. source ~/.bashrc   (ou relancez votre terminal)"
        echo "  2. polygone           (ouvre le TUI ou la CLI)"
        echo "  3. polygoned          (daemon arrière-plan, selon le service installé)"
        echo "  4. polygone-gateway   (site + API sur 127.0.0.1:8088)"
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

    # Releases vérifiées d'abord : aucune toolchain Rust n'est nécessaire
    # quand les binaires de la plateforme sont disponibles.
    local bin_dir
    bin_dir=$(resolve_binary_dir "$platform")

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
if [[ "${POLYGONE_INSTALL_TEST:-0}" != "1" ]]; then
    main "$@"
fi
