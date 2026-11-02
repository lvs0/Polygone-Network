"""Polygone — Phase 0 : franchir la porte.

Nœud B hébergé sur Modal. Objectif unique, du manifeste :

    « un octet traverse un câble entre deux machines réelles. »
    Critère de fin : sha256 reçu = sha256 émis, sur une machine différente.

Ce que ce fichier N'EST PAS :
  - Ce n'est pas un nœud du mesh LAN. Modal est du cloud : le mesh (UDP
    broadcast 255.255.255.255, mesh.rs) ne peut pas être prouvé ici.
    Le manifeste l'autorise : « Le mesh ne sera pas prouvé (LAN-only),
    mais le transport TCP, oui. »
  - Ce n'est pas un nœud de calcul. Il stocke et sert des blocs, c'est tout.

Protocole servi (crates/client/src/blocks.rs:21-23, TCP brut) :
    demandeur → serveur : GET <64 hex minuscules>\n
    serveur  → demandeur : HAVE <len>\n<len octets bruts>
    serveur  → demandeur : MISS\n

Le port 7643 est exposé en TCP brut non chiffré via modal.forward(..., unencrypted=True).

Déploiement :
    modal run phase0/modal_node.py
"""

import hashlib
import os
import socket
import subprocess
import threading
import time
from pathlib import Path

import modal

APP_NAME = "polygone-phase0"

# Le binaire `polygone` (client CLI) compilé en release. Il exige glibc >= 2.39
# (mesuré : objdump -T | GLIBC_2.39), donc PAS debian_slim/bookworm (glibc 2.36).
# On prend une base Debian trixie, dont la glibc est 2.41.
REPO = Path(__file__).resolve().parent.parent
BIN = REPO / "target" / "release" / "polygone"

image = (
    modal.Image.from_registry("debian:trixie-slim", add_python="3.12")
    .apt_install("ca-certificates")
    .add_local_file(str(BIN), "/usr/local/bin/polygone", copy=True)
)

app = modal.App(APP_NAME)

# Le bloc de test : 1 Mio déterministe. Son sha256 est calculable des deux côtés,
# ce qui rend la vérification indépendante du protocole Polygone lui-même.
BLOCK_BYTES = 1 * 1024 * 1024
BLOCK_SEED = b"polygone-phase0-block-v1"


def block_content() -> bytes:
    """1 Mio déterministe — même contenu sur A et sur B, sans transfert."""
    out = bytearray()
    counter = 0
    while len(out) < BLOCK_BYTES:
        out.extend(hashlib.sha256(BLOCK_SEED + counter.to_bytes(8, "big")).digest())
        counter += 1
    return bytes(out[:BLOCK_BYTES])


@app.function(image=image, cpu=1.0, memory=512, timeout=1800)
def node_b(serve_seconds: int = 900):
    """Nœud B : stocke un bloc, le sert en TCP brut, publie son adresse."""
    root = Path("/tmp/polygone-blocks")
    root.mkdir(parents=True, exist_ok=True)

    # 1. Écrire le bloc source sur le disque du conteneur.
    src = root / "source.bin"
    content = block_content()
    src.write_bytes(content)
    local_sha = hashlib.sha256(content).hexdigest()

    # 2. `polygone bloc mettre` → le magasin le range sous BLAKE3(contenu).
    env = dict(os.environ, HOME="/tmp/polygone-home")
    put = subprocess.run(
        ["polygone", "bloc", "mettre", str(src)],
        capture_output=True, text=True, env=env, timeout=120,
    )
    print("=== polygone bloc mettre ===")
    print("RC:", put.returncode)
    print("STDOUT:", put.stdout.strip())
    print("STDERR:", put.stderr.strip())

    # 3. Trouver le hash BLAKE3 du bloc rangé.
    #    Le magasin range sous `blocks/<2 premiers hex>/<hash sans les 2 premiers>`.
    #    On ne peut donc PAS prendre `p.name` tel quel : il faut recomposer.
    store = Path(env["HOME"]) / ".polygone" / "blocks"
    blake3_hash = None
    for p in store.rglob("*"):
        if p.is_file() and p.name != "source.bin":
            blake3_hash = p.parent.name + p.name
            break
    print("=== magasin ===")
    print("root:", store, "exists:", store.exists())
    print("blake3:", blake3_hash)
    assert blake3_hash is not None and len(blake3_hash) == 64, (
        f"hash BLAKE3 mal reconstruit : {blake3_hash!r} ({len(blake3_hash or '')} car.)"
    )

    # 4. Servir en TCP brut + tunnel Modal non chiffré.
    with modal.forward(7643, unencrypted=True) as tunnel:
        print("=== TUNNEL ===")
        print(f"TCP_HOST={tunnel.unencrypted_host}")
        print(f"TCP_PORT={tunnel.unencrypted_port}")
        print(f"BLOCK_SHA256={local_sha}")
        print(f"BLOCK_BLAKE3={blake3_hash}")
        print(f"BLOCK_SIZE={BLOCK_BYTES}")
        print("=== READY ===")
        import sys
        sys.stdout.flush()

        proc = subprocess.Popen(
            ["polygone", "bloc", "servir", "--port", "7643", "--ecoute", "0.0.0.0"],
            env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
        )
        deadline = time.time() + serve_seconds
        while time.time() < deadline and proc.poll() is None:
            time.sleep(1)
        proc.terminate()
        try:
            proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            proc.kill()

    return {"sha256": local_sha, "blake3": blake3_hash, "size": BLOCK_BYTES}


@app.local_entrypoint()
def main(serve_seconds: int = 900):
    """Lance le nœud B et garde le tunnel ouvert."""
    print("Lancement du nœud B sur Modal…")
    node_b.remote(serve_seconds=serve_seconds)
