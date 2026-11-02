#!/usr/bin/env bash
set -Eeuo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
TMP="$(mktemp -d -t polygone-installer-test.XXXXXX)"
cleanup() { rm -rf -- "$TMP"; }
trap cleanup EXIT INT TERM

mkdir -p "$TMP/fixture/bin" "$TMP/fixture/web" "$TMP/fakebin" "$TMP/tmp" "$TMP/install/bin" "$TMP/config" "$TMP/data" "$TMP/share/polygone" "$TMP/home"
export HOME="$TMP/home"
export TMPDIR="$TMP/tmp"
export POLYGONE_INSTALL_DIR="$TMP/install/bin"
export POLYGONE_CONFIG_DIR="$TMP/config"
export POLYGONE_DATA_DIR="$TMP/data"
export POLYGONE_SHARE_DIR="$TMP/share/polygone"
export POLYGONE_INSTALL_TEST=1
export POLYGONE_VERSION=2.0.0
export FIXTURE="$TMP/fixture"

for bin in polygone polygone-relay polygoned polygone-gateway; do
  cat > "$FIXTURE/bin/$bin" <<'BIN'
#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == "--version" ]]; then
  echo "${0##*/} 0.0.0-test"
  exit 0
fi
if [[ "${0##*/}" == "polygoned" && "${*: -1}" == "--gen-config" ]]; then
  previous=""
  for argument in "$@"; do
    if [[ "$previous" == "--config" ]]; then
      mkdir -p "$(dirname "$argument")"
      printf '[tier]\nname = \"default\"\n' > "$argument"
    fi
    previous="$argument"
  done
fi
BIN
  chmod 755 "$FIXTURE/bin/$bin"
done
printf '<!doctype html><title>fixture</title>\n' > "$FIXTURE/web/index.html"
printf '{}\n' > "$FIXTURE/web/manifest.webmanifest"
printf 'self.addEventListener("fetch", () => {});\n' > "$FIXTURE/web/sw.js"

make_archive() {
  local destination=$1 source_bin=$2
  tar -czf "$destination" -C "$source_bin" polygone polygone-relay polygoned polygone-gateway \
    -C "$FIXTURE" web
}

make_archive "$FIXTURE/good.tar.gz" "$FIXTURE/bin"
sha256sum "$FIXTURE/good.tar.gz" | awk '{print $1 "  polygone-linux-x86_64.tar.gz"}' > "$FIXTURE/good.SHA256SUMS"

printf 'unexpected\n' > "$FIXTURE/bin/unexpected"
make_archive "$FIXTURE/extra-base.tar.gz" "$FIXTURE/bin"
tar -czf "$FIXTURE/extra.tar.gz" -C "$FIXTURE/bin" polygone polygone-relay polygoned polygone-gateway unexpected \
  -C "$FIXTURE" web
sha256sum "$FIXTURE/extra.tar.gz" | awk '{print $1 "  polygone-linux-x86_64.tar.gz"}' > "$FIXTURE/extra.SHA256SUMS"
rm "$FIXTURE/bin/unexpected" "$FIXTURE/extra-base.tar.gz"

tar -czf "$FIXTURE/duplicate.tar.gz" -C "$FIXTURE/bin" polygone polygone polygone-relay polygoned polygone-gateway \
  -C "$FIXTURE" web
sha256sum "$FIXTURE/duplicate.tar.gz" | awk '{print $1 "  polygone-linux-x86_64.tar.gz"}' > "$FIXTURE/duplicate.SHA256SUMS"

mkdir -p "$FIXTURE/symlink/bin" "$FIXTURE/symlink/web"
ln -s /etc/passwd "$FIXTURE/symlink/bin/polygone"
for bin in polygone-relay polygoned polygone-gateway; do cp "$FIXTURE/bin/$bin" "$FIXTURE/symlink/bin/"; done
cp -a "$FIXTURE/web/." "$FIXTURE/symlink/web/"
tar -czf "$FIXTURE/symlink.tar.gz" -C "$FIXTURE/symlink" bin/polygone bin/polygone-relay bin/polygoned bin/polygone-gateway web
# Rename archive entries to the required root-level binaries; the symlink is
# kept at the root so the tar type verifier must reject it before extraction.
tar -czf "$FIXTURE/symlink.tar.gz" -C "$FIXTURE/symlink/bin" polygone polygone-relay polygoned polygone-gateway \
  -C "$FIXTURE/symlink" web
sha256sum "$FIXTURE/symlink.tar.gz" | awk '{print $1 "  polygone-linux-x86_64.tar.gz"}' > "$FIXTURE/symlink.SHA256SUMS"

# A deterministic curl substitute serves local fixtures for the real fetch_release function.
cat > "$TMP/fakebin/curl" <<'CURL'
#!/usr/bin/env bash
set -euo pipefail
out=""
url=""
while (($#)); do
  case "$1" in
    -o) out="$2"; shift 2 ;;
    *) url="$1"; shift ;;
  esac
done
case "$url" in
  */SHA256SUMS)
    case "${FAKE_MODE:-good}" in
      missing) exit 22 ;;
      mismatch) printf '%064d  polygone-linux-x86_64.tar.gz\n' 0 > "$out" ;;
      extra) cp "$FIXTURE/extra.SHA256SUMS" "$out" ;;
      symlink) cp "$FIXTURE/symlink.SHA256SUMS" "$out" ;;
      duplicate) cp "$FIXTURE/duplicate.SHA256SUMS" "$out" ;;
      *) cp "$FIXTURE/good.SHA256SUMS" "$out" ;;
    esac
    ;;
  */polygone-linux-x86_64.tar.gz)
    case "${FAKE_MODE:-good}" in
      extra) cp "$FIXTURE/extra.tar.gz" "$out" ;;
      symlink) cp "$FIXTURE/symlink.tar.gz" "$out" ;;
      duplicate) cp "$FIXTURE/duplicate.tar.gz" "$out" ;;
      *) cp "$FIXTURE/good.tar.gz" "$out" ;;
    esac
    ;;
  *) echo "unexpected URL: $url" >&2; exit 22 ;;
esac
CURL
chmod 755 "$TMP/fakebin/curl"
cat > "$TMP/fakebin/git" <<'GIT'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$@" > "$FAKE_GIT_LOG"
destination=""
for argument in "$@"; do destination="$argument"; done
mkdir -p "$destination/target/release" "$destination/web"
for bin in polygone polygone-relay polygoned polygone-gateway; do
  cat > "$destination/target/release/$bin" <<'BIN'
#!/usr/bin/env bash
[[ "${1:-}" == "--version" ]] && echo "${0##*/} 0.0.0-build-test"
BIN
  chmod 755 "$destination/target/release/$bin"
done
printf '<!doctype html><title>source fixture</title>\n' > "$destination/web/index.html"
printf '{}\n' > "$destination/web/manifest.webmanifest"
printf 'self.addEventListener("fetch", () => {});\n' > "$destination/web/sw.js"
GIT
cat > "$TMP/fakebin/cargo" <<'CARGO'
#!/usr/bin/env bash
set -euo pipefail
case "${1:-}" in
  build) exit 0 ;;
  metadata) printf '{"target_directory":"%s/target"}\n' "$PWD" ;;
  *) echo "unexpected fake cargo command: $*" >&2; exit 2 ;;
esac
CARGO
chmod 755 "$TMP/fakebin/git" "$TMP/fakebin/cargo"
export FAKE_GIT_LOG="$TMP/git.log"
export PATH="$TMP/fakebin:$PATH"

# The legacy URL wrapper must delegate to the canonical script without
# running installation when the test-only guard is set.
POLYGONE_INSTALL_TEST=1 bash "$ROOT/scripts/install.sh"

# Source definitions without running the interactive installer.
# shellcheck disable=SC1091
source "$ROOT/install.sh"
[[ "$MIN_RUST_VERSION" == "1.85.0" ]]
version_ge "1.85.0" "1.85.0"
! version_ge "1.84.1" "1.85.0"

# Success: command substitution must capture only the extracted directory path.
release_dir="$(fetch_release linux-x86_64 latest)"
[[ -d "$release_dir" && -x "$release_dir/polygone" && -x "$release_dir/polygone-relay" && -x "$release_dir/polygoned" && -x "$release_dir/polygone-gateway" ]]
[[ -f "$release_dir/web/index.html" && -f "$release_dir/web/manifest.webmanifest" && -f "$release_dir/web/sw.js" ]]
[[ "$release_dir" == "$TMP/tmp"/polygone-release.*/extract ]]
install_binaries "$release_dir"

# Prebuilt releases must not install/update Rust; source fallback must do so.
RUST_MARKER="$FIXTURE/rust-called"
ORIGINAL_BUILD_FROM_SOURCE="$(declare -f build_from_source)"
install_rust() { touch "$RUST_MARKER"; }
build_from_source() { printf '%s\n' "$FIXTURE/bin"; }
resolved="$(resolve_binary_dir linux-x86_64)"
[[ "$resolved" == "$TMP"/tmp/polygone-release.*/extract ]]
[[ -d "$resolved" ]]
[[ ! -e "$RUST_MARKER" ]] || { echo "Rust was installed for a verified prebuilt release" >&2; exit 1; }
rm -rf -- "$(dirname "$resolved")"
FAKE_MODE=missing
export FAKE_MODE
resolved="$(resolve_binary_dir linux-x86_64)"
[[ "$resolved" == "$FIXTURE/bin" ]]
[[ -f "$RUST_MARKER" ]]
unset FAKE_MODE
eval "$ORIGINAL_BUILD_FROM_SOURCE"

[[ -x "$POLYGONE_INSTALL_DIR/polygone" ]]
[[ -x "$POLYGONE_INSTALL_DIR/polygone-relay" ]]
[[ -x "$POLYGONE_INSTALL_DIR/polygoned" ]]
[[ -x "$POLYGONE_INSTALL_DIR/polygone-gateway" ]]
[[ -f "$POLYGONE_SHARE_DIR/web/index.html" ]]
[[ -f "$POLYGONE_SHARE_DIR/web/manifest.webmanifest" ]]
[[ -f "$POLYGONE_SHARE_DIR/web/sw.js" ]]
[[ -L "$POLYGONE_INSTALL_DIR/polygone-client" ]]
generate_config
[[ -f "$POLYGONE_CONFIG_DIR/daemon.toml" ]]
verify_install >/dev/null
[[ ! -d "$(dirname "$release_dir")" ]]

# Source fallback must return a live path, use the exact v-prefixed tag, and
# remain available until install_binaries has copied the files.
build_dir="$(build_from_source)"
[[ -x "$build_dir/polygone" && -x "$build_dir/polygone-relay" && -x "$build_dir/polygoned" && -x "$build_dir/polygone-gateway" ]]
[[ -f "$build_dir/web/index.html" && -f "$build_dir/web/manifest.webmanifest" && -f "$build_dir/web/sw.js" ]]
grep -Fxq -- '--branch' "$FAKE_GIT_LOG"
grep -Fxq 'v2.0.0' "$FAKE_GIT_LOG"
build_root="${build_dir%%/polygone/target/release}"
install_binaries "$build_dir"
[[ ! -d "$build_root" ]]

# Missing or invalid checksum must never yield an installable directory.
for mode in missing mismatch extra symlink duplicate; do
  export FAKE_MODE="$mode"
  if fetch_release linux-x86_64 latest >/dev/null; then
    echo "FAIL: fetch_release accepted mode=$mode" >&2
    exit 1
  fi
done

# Verify a bad checksum/manifest left no release temp trees behind.
if find "$TMP/tmp" -mindepth 1 -maxdepth 1 -type d -name 'polygone-release.*' | grep -q .; then
  echo "FAIL: release temp directory leaked" >&2
  exit 1
fi

echo "installer smoke: SHA256 fail-closed, four binaries + web bundle, archive path/type/duplicate allowlist, source fallback, daemon config, compatibility alias and temp cleanup: PASS"
