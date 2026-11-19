#!/usr/bin/env python3
"""P0 — redémarrage du relais en pleine session : que survit-il côté pairs ?

  1. relais R1 en place, tunnel unique, B enregistré, A envoie le marqueur 1
  2. R1 est tué (SIGKILL), puis un relais R2 démarre sur le même port,
     derrière le MÊME tunnel public : l'adresse ne change pas
  3. on mesure : état de l'écouteur de B, présence de son slot, sort d'un envoi
     juste après le redémarrage, puis après réenregistrement
  4. A renvoie le marqueur 2 avec la même identité

Ce que cela mesure : la perte d'état du relais (slots en mémoire), le sort du
processus d'écoute côté pair, et la survie de l'identité et de l'ancre de
confiance, qui sont locales.
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

app = modal.App("polygone-p0-relay-restart")


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


def _wait_key(box: modal.Dict, key: str, timeout: float) -> object:
    deadline = time.time() + timeout
    while time.time() < deadline:
        value = box.get(key)
        if value:
            return value
        time.sleep(0.3)
    return None


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

    lines1: list[str] = []
    relay1 = subprocess.Popen(
        ["/opt/polygone/polygone-relay", "--port", str(PORT), "--quiet"],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    threading.Thread(target=_drain, args=(relay1, lines1), daemon=True).start()
    if not _wait_port(PORT):
        print("RELAY_FAIL=" + " | ".join(lines1[-5:]), flush=True)
        box.put("error", "relais indisponible")
        return {"error": "relais indisponible"}
    print(f"RELAY1_PID={relay1.pid}", flush=True)

    result: dict = {"role": "relay", **facts, "relay1_pid": relay1.pid}
    with modal.forward(PORT, unencrypted=True) as tunnel:
        addr = "%s:%s" % tunnel.tcp_socket
        box.put("relay", addr)
        print(f"TUNNEL={tunnel.tcp_socket[0]} {tunnel.tcp_socket[1]}", flush=True)

        if not _wait_key(box, "restart", hold_s):
            box.put("done", True)
        killed_at = time.time()
        relay1.kill()
        try:
            relay1.wait(timeout=10)
        except subprocess.TimeoutExpired:
            relay1.kill()
        print("RELAY1_KILLED=1", flush=True)

        time.sleep(1.0)
        lines2: list[str] = []
        relay2 = subprocess.Popen(
            ["/opt/polygone/polygone-relay", "--port", str(PORT), "--quiet"],
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
        )
        threading.Thread(target=_drain, args=(relay2, lines2), daemon=True).start()
        up2 = _wait_port(PORT, 20)
        print("RELAY2_PID=%d" % relay2.pid, flush=True)
        print("RELAY_RESTARTED=" + ("1" if up2 else "0"), flush=True)
        box.put("relay_up2", up2)

        _wait_key(box, "done", hold_s)
        result.update(
            {
                "addr": addr,
                "relay1_killed_at_epoch": round(killed_at, 3),
                "relay2_pid": relay2.pid,
                "restart_ok": up2,
                "relay2_alive": relay2.poll() is None,
                "relay1_log_tail": lines1[-6:],
                "relay2_log_tail": lines2[-6:],
            }
        )
    print("R_SUMMARY=" + json.dumps(result, ensure_ascii=False), flush=True)
    return result


@app.function(image=image, timeout=1200)
def peer_fn(marker1: str, marker2: str, addr: str, box: modal.Dict, wait_s: int = 240) -> dict:
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
    listener1 = _listener(addr, env, lines1)
    if not _wait_line(lines1, "en écoute via relay", 60):
        print("LISTEN_FAIL=" + " | ".join(lines1[-5:]), flush=True)
        box.put("error", "pas d'enregistrement initial")
        return {"error": "pas d'enregistrement initial"}
    print("B_LISTENING=1", flush=True)
    box.put("listening", True)

    seen1 = _wait_line(lines1, marker1, wait_s)
    print("B_SEEN_MARKER1=" + ("1" if seen1 else "0"), flush=True)
    anchor1 = next((l.strip() for l in lines1 if "NOUVEAU PAIR APPRIS" in l), "")

    baseline = len(lines1)
    box.put("restart", True)

    # Observation : le processus d'écoute survit-il au décès du relais ?
    died = False
    for _ in range(75):
        if listener1.poll() is not None:
            died = True
            break
        time.sleep(0.2)
    exit_code = listener1.poll()
    print("B_LISTENER_DIED_ON_RESTART=" + ("1" if died else "0"), flush=True)
    print("B_LISTENER_EXIT=" + str(exit_code), flush=True)
    auto = any("en écoute via relay" in l for l in lines1[baseline:])
    print("B_AUTO_RECONNECTED=" + ("1" if auto else "0"), flush=True)

    try:
        listener1.kill()
    except Exception:
        pass

    _wait_key(box, "relay_up2", 120)

    # Réenregistrement volontaire, même adresse, même identité.
    lines2: list[str] = []
    listener2 = _listener(addr, env, lines2)
    re_registered = _wait_line(lines2, "en écoute via relay", 90)
    print("B_REREGISTERED_SAME_ADDR=" + ("1" if re_registered else "0"), flush=True)
    print("B_REREGISTER_LOG=" + " | ".join(lines2[-4:]), flush=True)
    box.put("reregistered", re_registered)

    relearned = any("NOUVEAU PAIR APPRIS" in l for l in lines2)
    print("B_ANCHOR_RELEARNED=" + ("1" if relearned else "0"), flush=True)

    seen2 = _wait_line(lines2, marker2, wait_s) if re_registered else False
    print("B_SEEN_MARKER2=" + ("1" if seen2 else "0"), flush=True)
    print("B_LINES1_TAIL=" + " | ".join(lines1[-12:]), flush=True)
    print("B_LINES2_TAIL=" + " | ".join(lines2[-12:]), flush=True)

    summary = {
        "role": "peer",
        "b_id": b_id,
        "b_kem_len": len(b_kem),
        "addr": addr,
        "b_listener_died_on_restart": died,
        "b_listener_exit_code": exit_code,
        "b_auto_reconnected": auto,
        "b_reregistered_same_addr": re_registered,
        "b_anchor_relearned": relearned,
        "b_seen_marker1": seen1,
        "b_seen_marker2": seen2,
        "b_trust_anchor": anchor1,
        "listener2_alive": listener2.poll() is None,
        **facts,
    }
    print("B_TRUST_ANCHOR=" + anchor1, flush=True)
    print("B_SUMMARY=" + json.dumps(summary, ensure_ascii=False), flush=True)
    box.put("done", True)
    return summary


@app.local_entrypoint()
def main(marker1: str, marker2: str, wait_s: int = 240) -> None:
    box = modal.Dict.from_name("polygone-p0-relay-restart-" + marker1, create_if_missing=True)
    box.hydrate()

    relay_call = relay_fn.spawn(marker1, marker2, box)
    addr = _wait_key(box, "relay", 240)
    if not addr:
        print("ORCH_FAIL=aucune adresse", flush=True)
        return

    peer_call = peer_fn.spawn(marker1, marker2, addr, box)
    if not _wait_key(box, "listening", 240):
        print("ORCH_FAIL=B n'est pas en écoute", flush=True)
        return
    print("ORCH_READY=1", flush=True)

    peer_result = peer_call.get()
    relay_result = relay_call.get()
    print("LOCAL_RESULT=" + json.dumps({"peer": peer_result, "relay": relay_result}, ensure_ascii=False), flush=True)
