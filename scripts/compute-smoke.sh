#!/usr/bin/env bash
set -Eeuo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="$ROOT/target/debug"
TMP="$(mktemp -d -t polygone-compute-smoke.XXXXXX)"
RELAY_PID=""
GHOST_PID=""

cleanup() {
  set +e
  if [[ -n "$GHOST_PID" ]] && kill -0 "$GHOST_PID" 2>/dev/null; then
    kill -INT "$GHOST_PID" 2>/dev/null
    for _ in {1..30}; do
      kill -0 "$GHOST_PID" 2>/dev/null || break
      sleep 0.1
    done
    kill -TERM "$GHOST_PID" 2>/dev/null
    wait "$GHOST_PID" 2>/dev/null
  fi
  if [[ -n "$RELAY_PID" ]] && kill -0 "$RELAY_PID" 2>/dev/null; then
    kill -TERM "$RELAY_PID" 2>/dev/null
    wait "$RELAY_PID" 2>/dev/null
  fi
  rm -rf -- "$TMP"
}
trap cleanup EXIT INT TERM

mkdir -p "$TMP/ghost" "$TMP/borrower"
export RUST_LOG=warn

cargo build --manifest-path "$ROOT/Cargo.toml" -p polygone-client -p polygone-relay

PORT="$(python3 - <<'PY'
import socket
s = socket.socket()
s.bind(("127.0.0.1", 0))
print(s.getsockname()[1])
s.close()
PY
)"
RELAY="127.0.0.1:$PORT"

"$BIN/polygone-relay" --port "$PORT" >"$TMP/relay.log" 2>&1 &
RELAY_PID=$!
python3 - "$PORT" <<'PY'
import socket, sys, time
port = int(sys.argv[1])
deadline = time.time() + 8
while time.time() < deadline:
    try:
        with socket.create_connection(("127.0.0.1", port), timeout=0.2):
            raise SystemExit(0)
    except OSError:
        time.sleep(0.1)
raise SystemExit("relay did not open its TCP port")
PY

GHOST_ID="$(HOME="$TMP/ghost" "$BIN/polygone" id)"
BORROWER_ID="$(HOME="$TMP/borrower" "$BIN/polygone" id)"

HOME="$TMP/ghost" "$BIN/polygone" ecouter --relay "$RELAY" --compute >"$TMP/ghost.log" 2>&1 &
GHOST_PID=$!
for _ in {1..50}; do
  grep -q "prêt de compute actif" "$TMP/ghost.log" 2>/dev/null && break
  kill -0 "$GHOST_PID" 2>/dev/null || { cat "$TMP/ghost.log"; exit 1; }
  sleep 0.1
done
grep -q "prêt de compute actif" "$TMP/ghost.log" || {
  cat "$TMP/ghost.log"
  echo "ghost did not become ready" >&2
  exit 1
}

SUCCESS_OUTPUT="$(HOME="$TMP/borrower" "$BIN/polygone" compute \
  --executer "printf 'POLYGONE_COMPUTE_SMOKE_OK'" \
  --emprunter "$GHOST_ID" --via "$RELAY" 2>&1)"
printf '%s\n' "$SUCCESS_OUTPUT"
grep -q "POLYGONE_COMPUTE_SMOKE_OK" <<<"$SUCCESS_OUTPUT"

if HOME="$TMP/borrower" "$BIN/polygone" compute \
  --executer "echo POLYGONE_EXPECTED_FAILURE >&2; exit 23" \
  --emprunter "$GHOST_ID" --via "$RELAY" >"$TMP/failure.log" 2>&1; then
  cat "$TMP/failure.log"
  echo "a non-zero remote command was incorrectly reported as success" >&2
  exit 1
fi
grep -q "refusé/échoué" "$TMP/failure.log"
cat "$TMP/failure.log"

python3 - "$TMP" "$GHOST_ID" "$BORROWER_ID" <<'PY'
import json, pathlib, sys
root = pathlib.Path(sys.argv[1])
ghost_id, borrower_id = sys.argv[2:]
borrower = json.loads((root / "borrower/.polygone/reputation.json").read_text())
ghost = json.loads((root / "ghost/.polygone/reputation.json").read_text())
assert borrower["nodes"][ghost_id]["ok"] >= 1
assert borrower["nodes"][ghost_id]["fail"] >= 1
assert ghost["nodes"][borrower_id]["ok"] >= 1
assert ghost["nodes"][borrower_id]["fail"] >= 1
borrower_peers = json.loads((root / "borrower/.polygone/peers.json").read_text())
ghost_peers = json.loads((root / "ghost/.polygone/peers.json").read_text())
assert ghost_id in borrower_peers
assert borrower_id in ghost_peers
print("compute smoke: signed success + signed failure + both local reputation ledgers + TOFU persistence: PASS")
PY
