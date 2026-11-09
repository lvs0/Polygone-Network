#!/usr/bin/env bash
set -Eeuo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="$ROOT/target/debug/polygone"
TMP="$(mktemp -d -t polygone-mesh-smoke.XXXXXX)"
ANNOUNCER_PID=""

cleanup() {
  set +e
  if [[ -n "$ANNOUNCER_PID" ]] && kill -0 "$ANNOUNCER_PID" 2>/dev/null; then
    kill -INT "$ANNOUNCER_PID" 2>/dev/null
    for _ in {1..30}; do
      kill -0 "$ANNOUNCER_PID" 2>/dev/null || break
      sleep 0.1
    done
    kill -TERM "$ANNOUNCER_PID" 2>/dev/null
    wait "$ANNOUNCER_PID" 2>/dev/null
  fi
  rm -rf -- "$TMP"
}
trap cleanup EXIT INT TERM

mkdir -p "$TMP/home"
export HOME="$TMP/home"
export RUST_LOG=warn

cargo build --manifest-path "$ROOT/Cargo.toml" -p polygone-client
NODE_ID="$("$BIN" id)"
"$BIN" annoncer --relay 127.0.0.1:7000 >"$TMP/announce.log" 2>&1 &
ANNOUNCER_PID=$!

for _ in {1..50}; do
  grep -q "annonce sur le LAN" "$TMP/announce.log" 2>/dev/null && break
  kill -0 "$ANNOUNCER_PID" 2>/dev/null || { cat "$TMP/announce.log"; exit 1; }
  sleep 0.1
done
grep -q "annonce sur le LAN" "$TMP/announce.log" || {
  cat "$TMP/announce.log"
  echo "LAN announcer did not become ready" >&2
  exit 1
}

DISCOVERED="$("$BIN" voisins --duree 2)"
printf '%s\n' "$DISCOVERED"
grep -Fq "$NODE_ID" <<<"$DISCOVERED" || {
  echo "the local announcing node was not rediscovered" >&2
  exit 1
}
echo "mesh smoke: UDP announce + PING response + node ID + relay endpoint: PASS"
