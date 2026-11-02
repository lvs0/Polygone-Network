#!/usr/bin/env bash
set -Eeuo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="${POLYGONE_GATEWAY_BIN:-$ROOT/target/release/polygone-gateway}"
TMP="$(mktemp -d -t polygone-gateway-smoke.XXXXXX)"
PID=""
cleanup() {
  set +e
  if [[ -n "$PID" ]] && kill -0 "$PID" 2>/dev/null; then
    kill -INT "$PID" 2>/dev/null
    for _ in {1..20}; do
      kill -0 "$PID" 2>/dev/null || break
      sleep 0.1
    done
    kill -TERM "$PID" 2>/dev/null
    wait "$PID" 2>/dev/null
  fi
  rm -rf -- "$TMP"
}
trap cleanup EXIT INT TERM

if [[ ! -x "$BIN" ]]; then
  cargo build --manifest-path "$ROOT/Cargo.toml" --release -p polygone-gateway
fi
[[ -x "$BIN" ]] || { echo "Gateway binary missing: $BIN" >&2; exit 1; }

mkdir -p "$TMP/home/.local/bin" "$TMP/home/.local/share/polygone/web" "$TMP/work"
install -m 0755 "$BIN" "$TMP/home/.local/bin/polygone-gateway"
cp -a "$ROOT/web/." "$TMP/home/.local/share/polygone/web/"

VERSION="$("$TMP/home/.local/bin/polygone-gateway" --version)"
grep -q '^polygone-gateway ' <<<"$VERSION"

python3 - "$TMP" <<'PY'
import http.client
import json
import os
import pathlib
import signal
import socket
import subprocess
import sys
import time

root = pathlib.Path(sys.argv[1])
home = root / "home"
work = root / "work"
binary = home / ".local/bin/polygone-gateway"

def free_port():
    sock = socket.socket()
    sock.bind(("127.0.0.1", 0))
    port = sock.getsockname()[1]
    sock.close()
    return port

port = free_port()
env = os.environ.copy()
env.pop("POLYGONE_SITE_DIR", None)
env["HOME"] = str(home)
env["POLYGONE_GATEWAY_ADDR"] = f"127.0.0.1:{port}"
env["RUST_LOG"] = "warn"
log = (root / "gateway.log").open("w")
server = subprocess.Popen([str(binary)], cwd=work, env=env, stdout=log, stderr=subprocess.STDOUT)
try:
    deadline = time.monotonic() + 8
    response = None
    while time.monotonic() < deadline:
        if server.poll() is not None:
            raise RuntimeError((root / "gateway.log").read_text(errors="replace"))
        try:
            conn = http.client.HTTPConnection("127.0.0.1", port, timeout=0.5)
            conn.request("GET", "/")
            response = conn.getresponse()
            html = response.read().decode("utf-8", errors="replace")
            conn.close()
            if response.status == 200:
                break
        except OSError:
            time.sleep(0.1)
    assert response is not None and response.status == 200, "installed site was not served"
    assert "Polygone" in html, html[:500]

    conn = http.client.HTTPConnection("127.0.0.1", port, timeout=2)
    conn.request("GET", "/api/v1/network/status")
    status = conn.getresponse()
    payload = json.loads(status.read())
    conn.close()
    assert status.status == 200, status.status
    assert isinstance(payload["nodes"], int)
    assert isinstance(payload["uptime_seconds"], int)
    print("gateway smoke: installed executable resolves bundled web from a different CWD; HTTP page + API status: PASS")
finally:
    if server.poll() is None:
        server.send_signal(signal.SIGINT)
        try:
            server.wait(timeout=3)
        except subprocess.TimeoutExpired:
            server.kill()
            server.wait()
    log.close()

# The binary must refuse public binds until TLS and mutual authentication exist.
public_env = env.copy()
public_env["POLYGONE_GATEWAY_ADDR"] = f"0.0.0.0:{free_port()}"
rejected = subprocess.run([str(binary)], cwd=work, env=public_env, text=True, capture_output=True, timeout=5)
assert rejected.returncode != 0, "Gateway unexpectedly accepted a public bind"
assert "écoute publique refusée" in rejected.stderr, rejected.stderr
print("gateway smoke: public HTTP bind refused: PASS")
PY
