#!/usr/bin/env python3
"""P0 — preuve réseau sur deux hôtes distincts, sans second ordinateur personnel.

    nœud A (machine locale)  →  relais + nœud B (conteneur Modal)

Le conteneur Modal est un noyau distinct, avec son horloge, sa carte réseau et
son chemin WAN. Le relais écoute dans le conteneur et n'est joignable depuis
l'extérieur que par un tunnel TCP brut (`modal.forward(port, unencrypted=True)`).

Sortie : lignes préfixées, consommées par `scripts/p0-modal-two-host.sh` :
    RELAY_READY=..., B_ID=..., B_KEM=..., TUNNEL=host port,
    B_SEEN_MARKER=1, B_TRUST_ANCHOR=..., SUMMARY={...}

Ce que cette preuve établit : le protocole AUTH_INIT/CHALLENGE/AUTH_RESP tient
entre deux noyaux séparés par un WAN, avec deux horloges distinctes.
Ce qu'elle n'établit pas : la traversée de NAT résidentiel (aucun pair n'est
derrière une box), ni la résistance à la perte de paquets à grande échelle.
"""

from __future__ import annotations

import json
import os
import platform
import re
import socket
import subprocess
import sys
import threading
import time
from pathlib import Path

import modal

ROOT = Path(__file__).resolve().parent.parent
BIN_DIR = ROOT / "target" / "debug"
PORT = 7000

# glibc de l'hôte de compilation : 2.44. L'image doit être au moins aussi récente
# que la version de glibc réellement exigée par le binaire (vérifiée par
# `objdump -T | grep GLIBC_`), sinon le conteneur refuse de démarrer le binaire.
image = (
    modal.Image.from_registry("ubuntu:24.04", add_python="3.12")
    # copy=True : les binaires sont figés dans l'image, ce qui permet encore une
    # étape de build après (Modal interdit add_local_* avant un run_commands).
    .add_local_file(BIN_DIR / "polygone", "/opt/polygone/polygone", copy=True)
    .add_local_file(BIN_DIR / "polygone-relay", "/opt/polygone/polygone-relay", copy=True)
    .run_commands("chmod 0755 /opt/polygone/polygone /opt/polygone/polygone-relay")
)

app = modal.App("polygone-p0-two-host")


def _wait_port(port: int, timeout: float = 30.0) -> bool:
    deadline = time.time() + timeout
    while time.time() < deadline:
        with socket.socket() as s:
            s.settimeout(0.5)
            if s.connect_ex(("127.0.0.1", port)) == 0:
                return True
        time.sleep(0.1)
    return False


def _run(argv: list[str], env: dict[str, str]) -> str:
    proc = subprocess.run(argv, env=env, capture_output=True, text=True)
    return (proc.stdout or "") + (proc.stderr or "")


def _drain(proc: subprocess.Popen, sink: list[str]) -> None:
    assert proc.stdout is not None
    for line in proc.stdout:
        sink.append(line.rstrip("\n"))


@app.function(image=image, timeout=1200, scaledown_window=120)
def two_host(marker: str, wait_s: int = 240) -> dict:
    home_b = Path("/tmp/b")
    home_b.mkdir(parents=True, exist_ok=True)
    env_b = {**os.environ, "HOME": str(home_b), "RUST_BACKTRACE": "1"}

    relay_lines: list[str] = []
    relay = subprocess.Popen(
        ["/opt/polygone/polygone-relay", "--port", str(PORT), "--quiet"],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        env=env_b,
    )
    threading.Thread(target=_drain, args=(relay, relay_lines), daemon=True).start()

    if not _wait_port(PORT):
        print("RELAY_FAIL=" + " | ".join(relay_lines[-5:]), flush=True)
        return {"error": "le relais n'écoute pas dans le conteneur"}

    b_id_raw = _run(["/opt/polygone/polygone", "id"], env_b)
    b_kem_raw = _run(["/opt/polygone/polygone", "clef"], env_b)
    m_id = re.search(r"[0-9a-f]{32}", b_id_raw)
    m_kem = re.search(r"[0-9a-f]{3000,}", b_kem_raw)
    if not m_id or not m_kem:
        print("IDENTITY_FAIL=" + (b_id_raw + b_kem_raw)[:500], flush=True)
        return {"error": "identité de B illisible"}
    b_id, b_kem = m_id.group(0), m_kem.group(0)

    print(f"RELAY_READY=pid {relay.pid} port {PORT}", flush=True)
    print(f"B_ID={b_id}", flush=True)
    print(f"B_KEM={b_kem}", flush=True)

    b_lines: list[str] = []
    listener = subprocess.Popen(
        ["/opt/polygone/polygone", "ecouter", "--relay", f"127.0.0.1:{PORT}"],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        env=env_b,
    )
    threading.Thread(target=_drain, args=(listener, b_lines), daemon=True).start()

    deadline = time.time() + 30
    while time.time() < deadline and not any("en écoute via relay" in l for l in b_lines):
        time.sleep(0.2)
    if not any("en écoute via relay" in l for l in b_lines):
        print("LISTEN_FAIL=" + " | ".join(b_lines[-5:]), flush=True)
        return {"error": "B ne s'est pas enregistré auprès du relais"}

    started = time.time()
    seen = False
    anchor = ""
    with modal.forward(PORT, unencrypted=True) as tunnel:
        host, pub_port = tunnel.tcp_socket
        print(f"TUNNEL={host} {pub_port}", flush=True)
        limit = time.time() + wait_s
        while time.time() < limit:
            if any(marker in l for l in b_lines):
                seen = True
                break
            if listener.poll() is not None:
                break
            time.sleep(0.2)
        for line in b_lines:
            if "NOUVEAU PAIR APPRIS" in line:
                anchor = line.strip()

    elapsed = round(time.time() - started, 3)
    kernel = platform.release()
    btime = ""
    try:
        for line in Path("/proc/stat").read_text().splitlines():
            if line.startswith("btime "):
                btime = line.split()[1]
    except OSError:
        pass

    summary = {
        "marker": marker,
        "b_id": b_id,
        "b_kem_len": len(b_kem),
        "b_seen_marker": seen,
        "b_trust_anchor": anchor,
        "b_kernel": kernel,
        "b_boot_time": btime,
        "b_uptime_clock": round(float(Path("/proc/uptime").read_text().split()[0]), 1),
        "container_hostname": socket.gethostname(),
        "cpu_count": os.cpu_count(),
        "tunnel_host": host,
        "tunnel_port": pub_port,
        "elapsed_s": elapsed,
        "listener_alive": listener.poll() is None,
    }
    print("B_SEEN_MARKER=" + ("1" if seen else "0"), flush=True)
    print("B_TRUST_ANCHOR=" + anchor, flush=True)
    print("SUMMARY=" + json.dumps(summary, ensure_ascii=False), flush=True)
    return summary


@app.local_entrypoint()
def main(marker: str, wait_s: int = 240) -> None:
    result = two_host.remote(marker, wait_s)
    print("LOCAL_RESULT=" + json.dumps(result, ensure_ascii=False), flush=True)
