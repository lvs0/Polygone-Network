#!/usr/bin/env python3
"""P0 — TROIS noyaux distincts : A local, relais R, nœud B.

    A (machine locale)  ──WAN──▶  R (conteneur Modal n°1)
    B (conteneur Modal n°2) ──WAN──▶ R

R et B ne sont pas deux processus du même conteneur : ce sont deux conteneurs
Modal distincts, donc deux instances de noyau, deux horloges, deux piles réseau
et deux adresses IP. Aucun ne partage de namespace avec un autre.

Coordination : un `modal.Dict` sert de boîte aux lettres. Le relais y publie son
adresse publique, B y publie son identité, l'orchestrateur local attend les deux
avant de laisser A envoyer.

Lignes machine-lisibles consommées par `scripts/p0-modal-three-kernels.sh`.
"""

from __future__ import annotations

import json
import os
import platform
import re
import socket
import subprocess
import threading
import time
from pathlib import Path

import modal

ROOT = Path(__file__).resolve().parent.parent
BIN_DIR = ROOT / "target" / "debug"
PORT = 7000

image = (
    modal.Image.from_registry("ubuntu:24.04", add_python="3.12")
    .add_local_file(BIN_DIR / "polygone", "/opt/polygone/polygone", copy=True)
    .add_local_file(BIN_DIR / "polygone-relay", "/opt/polygone/polygone-relay", copy=True)
    .run_commands("chmod 0755 /opt/polygone/polygone /opt/polygone/polygone-relay")
)

app = modal.App("polygone-p0-three-kernels")


def _host_facts() -> dict:
    btime = ""
    try:
        for line in Path("/proc/stat").read_text().splitlines():
            if line.startswith("btime "):
                btime = line.split()[1]
    except OSError:
        pass
    uptime = 0.0
    try:
        uptime = float(Path("/proc/uptime").read_text().split()[0])
    except OSError:
        pass
    try:
        local_ip = socket.gethostbyname(socket.gethostname())
    except OSError:
        local_ip = ""
    boot_id = ""
    try:
        boot_id = Path("/proc/sys/kernel/random/boot_id").read_text().strip()
    except OSError:
        pass
    return {
        "kernel": platform.release(),
        "boot_time": btime,
        "uptime_s": round(uptime, 1),
        "hostname": socket.gethostname(),
        "local_ip": local_ip,
        "boot_id": boot_id,
        "cpu": os.cpu_count(),
    }


def _wait_port(port: int, timeout: float = 30.0) -> bool:
    deadline = time.time() + timeout
    while time.time() < deadline:
        with socket.socket() as s:
            s.settimeout(0.5)
            if s.connect_ex(("127.0.0.1", port)) == 0:
                return True
        time.sleep(0.1)
    return False


def _drain(proc: subprocess.Popen, sink: list[str]) -> None:
    assert proc.stdout is not None
    for line in proc.stdout:
        sink.append(line.rstrip("\n"))


@app.function(image=image, timeout=900)
def relay_fn(marker: str, box: modal.Dict, hold_s: int = 420) -> dict:
    facts = _host_facts()
    for key, value in facts.items():
        print(f"R_{key.upper()}={value}", flush=True)

    lines: list[str] = []
    relay = subprocess.Popen(
        ["/opt/polygone/polygone-relay", "--port", str(PORT), "--quiet"],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    threading.Thread(target=_drain, args=(relay, lines), daemon=True).start()
    if not _wait_port(PORT):
        print("RELAY_FAIL=" + " | ".join(lines[-5:]), flush=True)
        box.put("error", "relais indisponible")
        return {"error": "relais indisponible"}

    with modal.forward(PORT, unencrypted=True) as tunnel:
        host, port = tunnel.tcp_socket
        addr = f"{host}:{port}"
        box.put("relay", addr)
        print(f"TUNNEL={host} {port}", flush=True)
        deadline = time.time() + hold_s
        while time.time() < deadline and not box.get("done"):
            time.sleep(0.5)

    summary = {
        "role": "relay",
        "addr": addr,
        "relay_alive": relay.poll() is None,
        "relay_log_tail": lines[-4:],
        **facts,
    }
    print("R_SUMMARY=" + json.dumps(summary, ensure_ascii=False), flush=True)
    return summary


@app.function(image=image, timeout=900)
def peer_fn(marker: str, relay_addr: str, box: modal.Dict, wait_s: int = 300) -> dict:
    facts = _host_facts()
    for key, value in facts.items():
        print(f"B_{key.upper()}={value}", flush=True)

    home = Path("/tmp/b")
    home.mkdir(parents=True, exist_ok=True)
    env = {**os.environ, "HOME": str(home)}

    out_id = subprocess.run(["/opt/polygone/polygone", "id"], env=env, capture_output=True, text=True)
    out_kem = subprocess.run(["/opt/polygone/polygone", "clef"], env=env, capture_output=True, text=True)
    m_id = re.search(r"[0-9a-f]{32}", out_id.stdout + out_id.stderr)
    m_kem = re.search(r"[0-9a-f]{3000,}", out_kem.stdout + out_kem.stderr)
    if not (m_id and m_kem):
        print("IDENTITY_FAIL=" + (out_id.stdout + out_kem.stdout)[:300], flush=True)
        box.put("error", "identité B illisible")
        return {"error": "identité B illisible"}

    b_id, b_kem = m_id.group(0), m_kem.group(0)
    box.put("b_id", b_id)
    box.put("b_kem", b_kem)
    print("B_ID=" + b_id, flush=True)
    print("B_KEM=" + b_kem, flush=True)

    lines: list[str] = []
    listener = subprocess.Popen(
        ["/opt/polygone/polygone", "ecouter", "--relay", relay_addr],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        env=env,
    )
    threading.Thread(target=_drain, args=(listener, lines), daemon=True).start()

    deadline = time.time() + 60
    while (
        time.time() < deadline
        and not any("en écoute via relay" in l for l in lines)
        and listener.poll() is None
    ):
        time.sleep(0.2)
    if not any("en écoute via relay" in l for l in lines):
        print("LISTEN_FAIL=" + " | ".join(lines[-5:]), flush=True)
        box.put("error", "B ne s'est pas enregistré auprès du relais distant")
        return {"error": "B ne s'est pas enregistré"}

    started = time.time()
    seen = False
    deadline = time.time() + wait_s
    while time.time() < deadline:
        if any(marker in l for l in lines):
            seen = True
            break
        if listener.poll() is not None:
            break
        time.sleep(0.2)

    anchor = next((l.strip() for l in lines if "NOUVEAU PAIR APPRIS" in l), "")
    summary = {
        "role": "peer",
        "marker": marker,
        "b_id": b_id,
        "b_kem_len": len(b_kem),
        "b_seen_marker": seen,
        "b_trust_anchor": anchor,
        "elapsed_s": round(time.time() - started, 3),
        "relay_addr": relay_addr,
        "listener_alive": listener.poll() is None,
        **facts,
    }
    print("B_SEEN_MARKER=" + ("1" if seen else "0"), flush=True)
    print("B_TRUST_ANCHOR=" + anchor, flush=True)
    print("B_SUMMARY=" + json.dumps(summary, ensure_ascii=False), flush=True)
    box.put("done", True)
    return summary


@app.local_entrypoint()
def main(marker: str, wait_s: int = 300) -> None:
    box = modal.Dict.from_name(f"polygone-p0-{marker}", create_if_missing=True)
    box.hydrate()  # from_name rend un handle paresseux, non sérialisable avant hydratation

    relay_call = relay_fn.spawn(marker, box)
    addr = None
    deadline = time.time() + 240
    while time.time() < deadline:
        addr = box.get("relay")
        if addr:
            break
        time.sleep(0.5)
    if not addr:
        print("ORCH_FAIL=le relais n'a pas publié d'adresse publique", flush=True)
        return

    peer_call = peer_fn.spawn(marker, addr, box)
    b_id = b_kem = None
    deadline = time.time() + 240
    while time.time() < deadline:
        b_id = box.get("b_id")
        b_kem = box.get("b_kem")
        if b_id and b_kem:
            break
        time.sleep(0.5)
    if not (b_id and b_kem):
        print("ORCH_FAIL=B n'a pas publié son identité", flush=True)
        return

    print("ORCH_READY=1", flush=True)

    peer_result = peer_call.get()
    relay_result = relay_call.get()
    print(
        "LOCAL_RESULT=" + json.dumps({"peer": peer_result, "relay": relay_result}, ensure_ascii=False),
        flush=True,
    )
