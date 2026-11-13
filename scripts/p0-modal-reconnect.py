#!/usr/bin/env python3
"""P0 — coupure du chemin public en pleine session.

  1. tunnel n°1 → B s'enregistre → A envoie le marqueur 1 → B le reçoit
  2. le tunnel n°1 est fermé (le chemin public disparaît)
  3. tunnel n°2 sur un nouveau port → B se réenregistre, même identité
  4. A envoie le marqueur 2 via le tunnel n°2 → B doit le recevoir

Ce que cela prouve : la reprise après coupure côté destinataire, avec la même
identité, sans nouvelle identité ni nouvel appairage.
Ce que cela ne prouve pas : la reprise côté relais si le relais lui-même
redémarre (l'état des slots est en mémoire), ni la traversée de NAT résidentiel.
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

app = modal.App("polygone-p0-reconnect")


def _facts() -> dict:
    btime, boot_id, uptime = "", "", 0.0
    try:
        for line in Path("/proc/stat").read_text().splitlines():
            if line.startswith("btime "):
                btime = line.split()[1]
    except OSError:
        pass
    try:
        boot_id = Path("/proc/sys/kernel/random/boot_id").read_text().strip()
    except OSError:
        pass
    try:
        uptime = float(Path("/proc/uptime").read_text().split()[0])
    except OSError:
        pass
    return {
        "kernel": platform.release(),
        "boot_time": btime,
        "boot_id": boot_id,
        "uptime_s": round(uptime, 1),
        "hostname": socket.gethostname(),
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


def _wait_line(sink: list[str], needle: str, timeout: float) -> bool:
    deadline = time.time() + timeout
    while time.time() < deadline:
        if any(needle in l for l in sink):
            return True
        time.sleep(0.2)
    return False


def _listener(addr: str, env: dict, sink: list[str]) -> subprocess.Popen:
    proc = subprocess.Popen(
        ["/opt/polygone/polygone", "ecouter", "--relay", addr],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        env=env,
    )
    threading.Thread(target=_drain, args=(proc, sink), daemon=True).start()
    return proc


@app.function(image=image, timeout=1200)
def relay_fn(marker1: str, marker2: str, box: modal.Dict, hold_s: int = 420) -> dict:
    facts = _facts()
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
        box.put("error", "relais indisponible")
        print("RELAY_FAIL=" + " | ".join(lines[-5:]), flush=True)
        return {"error": "relais indisponible"}

    with modal.forward(PORT, unencrypted=True) as t1:
        addr1 = "%s:%s" % t1.tcp_socket
        box.put("relay", addr1)
        print(f"TUNNEL={t1.tcp_socket[0]} {t1.tcp_socket[1]}", flush=True)
        deadline = time.time() + hold_s
        while time.time() < deadline and not box.get("cut"):
            time.sleep(0.3)

    cut_at = time.time()
    print("CUT=1", flush=True)
    time.sleep(1.5)

    with modal.forward(PORT, unencrypted=True) as t2:
        addr2 = "%s:%s" % t2.tcp_socket
        box.put("relay2", addr2)
        print(f"TUNNEL2={t2.tcp_socket[0]} {t2.tcp_socket[1]}", flush=True)
        deadline = time.time() + hold_s
        while time.time() < deadline and not box.get("done"):
            time.sleep(0.3)

    summary = {
        "role": "relay",
        "addr1": addr1,
        "addr2": addr2,
        "cut_at_epoch": round(cut_at, 3),
        "relay_alive": relay.poll() is None,
        "relay_log_tail": lines[-6:],
        **facts,
    }
    print("R_SUMMARY=" + json.dumps(summary, ensure_ascii=False), flush=True)
    return summary


@app.function(image=image, timeout=1200)
def peer_fn(marker1: str, marker2: str, addr1: str, box: modal.Dict, wait_s: int = 240) -> dict:
    facts = _facts()
    for key, value in facts.items():
        print(f"B_{key.upper()}=" + str(value), flush=True)

    home = Path("/tmp/b")
    home.mkdir(parents=True, exist_ok=True)
    env = {**os.environ, "HOME": str(home)}

    out_id = subprocess.run(["/opt/polygone/polygone", "id"], env=env, capture_output=True, text=True)
    out_kem = subprocess.run(["/opt/polygone/polygone", "clef"], env=env, capture_output=True, text=True)
    m_id = re.search(r"[0-9a-f]{32}", out_id.stdout + out_id.stderr)
    m_kem = re.search(r"[0-9a-f]{3000,}", out_kem.stdout + out_kem.stderr)
    if not (m_id and m_kem):
        box.put("error", "identité B illisible")
        return {"error": "identité B illisible"}
    b_id, b_kem = m_id.group(0), m_kem.group(0)
    box.put("b_id", b_id)
    box.put("b_kem", b_kem)
    print("B_ID=" + b_id, flush=True)
    print("B_KEM=" + b_kem, flush=True)

    lines1: list[str] = []
    listener1 = _listener(addr1, env, lines1)
    if not _wait_line(lines1, "en écoute via relay", 60):
        print("LISTEN_FAIL=" + " | ".join(lines1[-5:]), flush=True)
        box.put("error", "B ne s'est pas enregistré (phase 1)")
        return {"error": "phase 1 : pas d'enregistrement"}
    box.put("listening", True)
    print("B_LISTENING=1", flush=True)

    seen1 = _wait_line(lines1, marker1, wait_s)
    print("B_SEEN_MARKER1=" + ("1" if seen1 else "0"), flush=True)
    anchor1 = next((l.strip() for l in lines1 if "NOUVEAU PAIR APPRIS" in l), "")

    # Demande la coupure : le relais ferme son tunnel.
    box.put("cut", True)

    addr2 = None
    deadline = time.time() + 240
    while time.time() < deadline:
        candidate = box.get("relay2")
        if candidate and candidate != addr1:
            addr2 = candidate
            break
        time.sleep(0.3)

    died = listener1.poll() is not None
    print("B_LISTENER_DIED=" + ("1" if died else "0"), flush=True)
    try:
        listener1.kill()
    except Exception:
        pass
    if not addr2:
        print("RECONNECT_FAIL=aucun second tunnel publié", flush=True)
        box.put("error", "pas de second tunnel")
        box.put("done", True)
        return {"error": "pas de second tunnel"}

    lines2: list[str] = []
    listener2 = _listener(addr2, env, lines2)
    reconnected = _wait_line(lines2, "en écoute via relay", 90)
    print("B_RECONNECTED=" + ("1" if reconnected else "0"), flush=True)
    if not reconnected:
        print("RECONNECT_FAIL=" + " | ".join(lines2[-6:]), flush=True)
        box.put("error", "réenregistrement refusé")
        box.put("done", True)
        return {"error": "réenregistrement refusé", "b_id": b_id, "addr1": addr1, "addr2": addr2}
    box.put("listening2", True)
    print("B_LISTENING2=1", flush=True)

    seen2 = _wait_line(lines2, marker2, wait_s)
    print("B_SEEN_MARKER2=" + ("1" if seen2 else "0"), flush=True)
    anchor2 = next((l.strip() for l in lines2 if "NOUVEAU PAIR APPRIS" in l), "")

    summary = {
        "role": "peer",
        "b_id": b_id,
        "b_kem_len": len(b_kem),
        "addr1": addr1,
        "addr2": addr2,
        "b_listener_died_on_cut": died,
        "b_reconnected": reconnected,
        "b_seen_marker1": seen1,
        "b_seen_marker2": seen2,
        "b_trust_anchor": anchor1,
        "b_anchor_relearned_after_cut": bool(anchor2),
        "listener2_alive": listener2.poll() is None,
        **facts,
    }
    print("B_TRUST_ANCHOR=" + anchor1, flush=True)
    print("B_SUMMARY=" + json.dumps(summary, ensure_ascii=False), flush=True)
    box.put("done", True)
    return summary


@app.local_entrypoint()
def main(marker1: str, marker2: str, wait_s: int = 240) -> None:
    box = modal.Dict.from_name("polygone-p0-reconnect-" + marker1, create_if_missing=True)
    box.hydrate()

    relay_call = relay_fn.spawn(marker1, marker2, box)
    addr1 = None
    deadline = time.time() + 240
    while time.time() < deadline:
        addr1 = box.get("relay")
        if addr1:
            break
        time.sleep(0.5)
    if not addr1:
        print("ORCH_FAIL=aucune adresse de relais", flush=True)
        return

    peer_call = peer_fn.spawn(marker1, marker2, addr1, box)
    deadline = time.time() + 240
    while time.time() < deadline:
        if box.get("b_id") and box.get("b_kem") and box.get("listening"):
            break
        time.sleep(0.5)
    if not (box.get("b_id") and box.get("listening")):
        print("ORCH_FAIL=B n'est pas prêt", flush=True)
        return

    print("ORCH_READY=1", flush=True)
    peer_result = peer_call.get()
    relay_result = relay_call.get()
    print("LOCAL_RESULT=" + json.dumps({"peer": peer_result, "relay": relay_result}, ensure_ascii=False), flush=True)
