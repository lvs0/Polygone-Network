#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
RUNNER="$SCRIPT_DIR/smoke-two-hosts.sh"
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/polygone-two-hosts-test.XXXXXX")"
trap 'rm -rf -- "$TMP_DIR"' EXIT

fail() {
    printf 'FAIL: %s\n' "$*" >&2
    exit 1
}

assert_eq() {
    local expected="$1" actual="$2" label="$3"
    [[ "$actual" == "$expected" ]] || fail "$label: attendu <$expected>, obtenu <$actual>"
}

assert_file_contains() {
    local file="$1" text="$2" label="$3"
    grep -Fq -- "$text" "$file" || fail "$label: <$text> absent de $file"
}

FAKE_BIN="$TMP_DIR/fake-polygone"
cat > "$FAKE_BIN" <<'FAKE'
#!/usr/bin/env bash
set -euo pipefail

store="${HOME:-.}/.polygone/peers.json"
marker="${store}.initialized"
command_name="${1:-}"
printf '%s\n' "$*" >> "$TEST_CALL_LOG"

case "$command_name" in
    initialiser-pairs)
        if [[ -e "$store" || -L "$store" || -e "$marker" || -L "$marker" ]]; then
            printf 'refusing to overwrite an existing trust-store path\n' >&2
            exit 90
        fi
        mkdir -p "$(dirname -- "$store")"
        printf '{}\n' > "$store"
        printf 'polygone-peers-store-v1\n' > "$marker"
        printf 'bootstrap stdout\n'
        printf 'bootstrap stderr\n' >&2
        ;;
    ecouter)
        [[ "${2:-}" == '--relay' && -n "${3:-}" ]] || exit 91
        case "$TEST_CASE" in
            fresh)
                [[ -f "$store" && -f "$marker" ]] || exit 92
                grep -Fxq 'polygone-peers-store-v1' "$marker" || exit 93
                ;;
            existing)
                [[ -f "$store" && -f "$marker" ]] || exit 94
                cmp -s "$store" "$TEST_EXPECTED_STORE" || exit 95
                ;;
            legacy)
                [[ -f "$store" && ! -e "$marker" ]] || exit 96
                cmp -s "$store" "$TEST_EXPECTED_STORE" || exit 97
                ;;
            orphan)
                [[ ! -e "$store" && -f "$marker" ]] || exit 98
                ;;
            corrupt)
                [[ -f "$store" && -f "$marker" ]] || exit 99
                cmp -s "$store" "$TEST_EXPECTED_STORE" || exit 100
                ;;
            *)
                printf 'unknown test case: %s\n' "$TEST_CASE" >&2
                exit 101
                ;;
        esac
        printf 'listener stdout: %s\n' "$TEST_CASE"
        printf 'listener stderr: %s\n' "$TEST_CASE" >&2
        exit "$TEST_LISTENER_EXIT"
        ;;
    *)
        printf 'unexpected command: %s\n' "$command_name" >&2
        exit 102
        ;;
esac
FAKE
chmod 700 "$FAKE_BIN"

run_listener() {
    local case_name="$1" home="$2" calls="$3" stdout="$4" stderr="$5" exit_code="$6" expected_store="${7:-}"
    local status=0
    if HOME="$home" \
        POLYGONE_BIN="$FAKE_BIN" \
        TEST_CASE="$case_name" \
        TEST_CALL_LOG="$calls" \
        TEST_LISTENER_EXIT="$exit_code" \
        TEST_EXPECTED_STORE="$expected_store" \
        bash "$RUNNER" listen 127.0.0.1:7000 >"$stdout" 2>"$stderr"; then
        status=0
    else
        status=$?
    fi
    RUN_STATUS="$status"
}

# A fresh HOME bootstraps the trust store before starting the listener.
FRESH_HOME="$TMP_DIR/fresh-home"
FRESH_CALLS="$TMP_DIR/fresh.calls"
run_listener fresh "$FRESH_HOME" "$FRESH_CALLS" "$TMP_DIR/fresh.out" "$TMP_DIR/fresh.err" 23
assert_eq 23 "$RUN_STATUS" 'fresh listener exit status'
[[ -f "$FRESH_HOME/.polygone/peers.json" ]] || fail 'fresh HOME did not get peers.json'
[[ -f "$FRESH_HOME/.polygone/peers.json.initialized" ]] || fail 'fresh HOME did not get initialization marker'
printf 'initialiser-pairs\necouter --relay 127.0.0.1:7000\n' > "$TMP_DIR/fresh.expected-calls"
cmp -s "$TMP_DIR/fresh.expected-calls" "$FRESH_CALLS" || fail 'fresh HOME command order is wrong'
assert_file_contains "$TMP_DIR/fresh.out" 'Premier démarrage' 'fresh stdout'
assert_file_contains "$TMP_DIR/fresh.out" 'bootstrap stdout' 'bootstrap stdout forwarding'
assert_file_contains "$TMP_DIR/fresh.err" 'bootstrap stderr' 'bootstrap stderr forwarding'
assert_file_contains "$TMP_DIR/fresh.out" 'listener stdout: fresh' 'listener stdout forwarding'
assert_file_contains "$TMP_DIR/fresh.err" 'listener stderr: fresh' 'listener stderr forwarding'

# An initialized store is used as-is and never reinitialized.
EXISTING_HOME="$TMP_DIR/existing-home"
mkdir -p "$EXISTING_HOME/.polygone"
printf '{"trusted-node":"trusted-key"}\n' > "$EXISTING_HOME/.polygone/peers.json"
printf 'polygone-peers-store-v1\n' > "$EXISTING_HOME/.polygone/peers.json.initialized"
cp "$EXISTING_HOME/.polygone/peers.json" "$TMP_DIR/existing.expected-store"
run_listener existing "$EXISTING_HOME" "$TMP_DIR/existing.calls" "$TMP_DIR/existing.out" "$TMP_DIR/existing.err" 31 "$TMP_DIR/existing.expected-store"
assert_eq 31 "$RUN_STATUS" 'existing-store listener exit status'
printf 'ecouter --relay 127.0.0.1:7000\n' > "$TMP_DIR/listen.expected-calls"
cmp -s "$TMP_DIR/listen.expected-calls" "$TMP_DIR/existing.calls" || fail 'existing store was unexpectedly initialized'
cmp -s "$TMP_DIR/existing.expected-store" "$EXISTING_HOME/.polygone/peers.json" || fail 'existing store was changed'

# A legacy JSON store without a marker is left for load_peers migration.
LEGACY_HOME="$TMP_DIR/legacy-home"
mkdir -p "$LEGACY_HOME/.polygone"
printf '{"legacy-node":"legacy-key"}\n' > "$LEGACY_HOME/.polygone/peers.json"
cp "$LEGACY_HOME/.polygone/peers.json" "$TMP_DIR/legacy.expected-store"
run_listener legacy "$LEGACY_HOME" "$TMP_DIR/legacy.calls" "$TMP_DIR/legacy.out" "$TMP_DIR/legacy.err" 32 "$TMP_DIR/legacy.expected-store"
assert_eq 32 "$RUN_STATUS" 'legacy-store listener exit status'
cmp -s "$TMP_DIR/listen.expected-calls" "$TMP_DIR/legacy.calls" || fail 'legacy store was unexpectedly initialized'
cmp -s "$TMP_DIR/legacy.expected-store" "$LEGACY_HOME/.polygone/peers.json" || fail 'legacy store bytes changed'
[[ ! -e "$LEGACY_HOME/.polygone/peers.json.initialized" ]] || fail 'runner touched legacy marker'

# An orphan marker is fail-closed; do not recreate its missing peers.json.
ORPHAN_HOME="$TMP_DIR/orphan-home"
mkdir -p "$ORPHAN_HOME/.polygone"
printf 'polygone-peers-store-v1\n' > "$ORPHAN_HOME/.polygone/peers.json.initialized"
run_listener orphan "$ORPHAN_HOME" "$TMP_DIR/orphan.calls" "$TMP_DIR/orphan.out" "$TMP_DIR/orphan.err" 41
assert_eq 41 "$RUN_STATUS" 'orphan-marker listener exit status'
cmp -s "$TMP_DIR/listen.expected-calls" "$TMP_DIR/orphan.calls" || fail 'orphan marker triggered rebootstrap'
[[ ! -e "$ORPHAN_HOME/.polygone/peers.json" ]] || fail 'orphan marker was silently reinitialized'

# A corrupt initialized store is preserved; listener failure remains visible.
CORRUPT_HOME="$TMP_DIR/corrupt-home"
mkdir -p "$CORRUPT_HOME/.polygone"
printf 'not-json\n' > "$CORRUPT_HOME/.polygone/peers.json"
printf 'polygone-peers-store-v1\n' > "$CORRUPT_HOME/.polygone/peers.json.initialized"
cp "$CORRUPT_HOME/.polygone/peers.json" "$TMP_DIR/corrupt.expected-store"
run_listener corrupt "$CORRUPT_HOME" "$TMP_DIR/corrupt.calls" "$TMP_DIR/corrupt.out" "$TMP_DIR/corrupt.err" 43 "$TMP_DIR/corrupt.expected-store"
assert_eq 43 "$RUN_STATUS" 'corrupt-store listener exit status'
cmp -s "$TMP_DIR/listen.expected-calls" "$TMP_DIR/corrupt.calls" || fail 'corrupt store was unexpectedly initialized'
cmp -s "$TMP_DIR/corrupt.expected-store" "$CORRUPT_HOME/.polygone/peers.json" || fail 'corrupt store bytes changed'

printf 'PASS: listen runner bootstrap, existing/legacy/orphan/corrupt store handling, stdout/stderr, and exit-code propagation.\n'
