#!/usr/bin/env python3
"""MSG.6 — mesure le débit réel d'un envoi Polygone via le relais.

Le protocole est celui du produit : identités ML-KEM/ML-DSA réelles, relais
réel, chiffrement, signature, fragmentation 4/7 et écriture du fichier reçu.
La mesure est end-to-end et inclut le démarrage d'un processus `polygone` par
envoi ; elle ne prétend donc pas isoler le seul débit du relais.

Sortie : un rapport Markdown daté, plus un résumé sur stdout. Échec si un seul
fichier reçu manque ou si son SHA-256 diffère.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import socket
import statistics
import subprocess
import sys
import tempfile
import time
from datetime import date
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_BIN = ROOT / "target" / "debug" / "polygone"
DEFAULT_RELAY = ROOT / "target" / "debug" / "polygone-relay"


def fail(message: str) -> "NoReturn":
    print(f"✖ {message}", file=sys.stderr)
    raise SystemExit(1)


def wait_port(port: int, timeout: float = 10.0) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
            sock.settimeout(0.2)
            if sock.connect_ex(("127.0.0.1", port)) == 0:
                return
        time.sleep(0.05)
    fail(f"relais non joignable sur 127.0.0.1:{port}")


def wait_for(path: Path, timeout: float = 30.0) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if path.is_file() and path.stat().st_size > 0:
            return
        time.sleep(0.05)
    fail(f"fichier reçu absent : {path}")


def run_env(home: Path) -> dict[str, str]:
    env = os.environ.copy()
    env["HOME"] = str(home)
    env.setdefault("RUST_LOG", "warn")
    return env


def parse_key(output: str) -> str:
    for line in output.splitlines():
        line = line.strip()
        if line and not line.startswith("#"):
            return line
    fail("`polygone clef` n'a produit aucune clé")


def main() -> int:
    parser = argparse.ArgumentParser(description="Mesure le débit réel de Polygone (MSG.6)")
    parser.add_argument("--runs", type=int, default=10)
    parser.add_argument("--size", type=int, default=12 * 1024)
    parser.add_argument("--bin", type=Path, default=DEFAULT_BIN)
    parser.add_argument("--relay-bin", type=Path, default=DEFAULT_RELAY)
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()

    if args.runs < 1 or args.size < 1:
        fail("--runs et --size doivent être strictement positifs")
    for binary in (args.bin, args.relay_bin):
        if not binary.is_file() or not os.access(binary, os.X_OK):
            fail(f"binaire absent ou non exécutable : {binary} (cargo build d'abord)")

    report = args.report or (ROOT / "reports" / f"msg-throughput-audit-{date.today().isoformat()}.md")
    report.parent.mkdir(parents=True, exist_ok=True)

    payload = bytes((index * 17 + 3) % 251 for index in range(args.size))
    payload_hash = hashlib.sha256(payload).hexdigest()

    with tempfile.TemporaryDirectory(prefix="polygone-throughput.") as tmp_name:
        tmp = Path(tmp_name)
        tx_home, rx_home = tmp / "tx", tmp / "rx"
        tx_home.mkdir()
        rx_home.mkdir()

        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as probe:
            probe.bind(("127.0.0.1", 0))
            port = probe.getsockname()[1]

        relay_log = (tmp / "relay.log").open("w")
        relay = subprocess.Popen(
            [str(args.relay_bin), "--port", str(port)],
            cwd=ROOT,
            stdout=relay_log,
            stderr=subprocess.STDOUT,
        )
        receiver_log = (tmp / "receiver.log").open("w")
        receiver: subprocess.Popen[bytes] | None = None
        try:
            wait_port(port)
            receiver = subprocess.Popen(
                [str(args.bin), "ecouter", "--relay", f"127.0.0.1:{port}"],
                cwd=ROOT,
                env=run_env(rx_home),
                stdout=receiver_log,
                stderr=subprocess.STDOUT,
            )
            time.sleep(1.0)

            # La clé et le node_id doivent être ceux du RÉCEPTEUR. Utiliser
            # ceux de l'émetteur chiffre vers soi-même et le récepteur ne peut
            # pas déchiffrer (le relais abandonne alors le pair).
            key = parse_key(
                subprocess.run(
                    [str(args.bin), "clef"],
                    cwd=ROOT,
                    env=run_env(rx_home),
                    capture_output=True,
                    text=True,
                    check=True,
                ).stdout
            )
            node = subprocess.run(
                [str(args.bin), "id"],
                cwd=ROOT,
                env=run_env(rx_home),
                capture_output=True,
                text=True,
                check=True,
            ).stdout.strip().splitlines()[0]
            if not node:
                fail("`polygone id` n'a produit aucun node_id")

            samples: list[dict[str, object]] = []
            for index in range(args.runs):
                name = f"payload-{index:02d}.bin"
                source = tmp / name
                source.write_bytes(payload)
                started = time.perf_counter()
                sent = subprocess.run(
                    [
                        str(args.bin),
                        "envoyer",
                        "--via",
                        f"127.0.0.1:{port}",
                        "--a",
                        node,
                        "-d",
                        key,
                        "--fichier",
                        str(source),
                    ],
                    cwd=ROOT,
                    env=run_env(tx_home),
                    capture_output=True,
                    text=True,
                    timeout=120,
                )
                elapsed = time.perf_counter() - started
                if sent.returncode != 0:
                    fail(f"envoi {index} échoué (code {sent.returncode}) : {sent.stderr or sent.stdout}")
                received = rx_home / ".polygone" / "received" / name
                wait_for(received)
                actual = hashlib.sha256(received.read_bytes()).hexdigest()
                if actual != payload_hash:
                    fail(f"SHA-256 différent pour {name} : {actual} != {payload_hash}")
                samples.append({"run": index + 1, "seconds": elapsed, "mib_s": (args.size / 1048576) / elapsed})
        finally:
            for process in (receiver, relay):
                if process is not None and process.poll() is None:
                    process.terminate()
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait()
            receiver_log.close()
            relay_log.close()

    rates = [float(sample["mib_s"]) for sample in samples]
    seconds = [float(sample["seconds"]) for sample in samples]
    median = statistics.median(rates)
    mean = statistics.mean(rates)
    stdev = statistics.pstdev(rates)
    lines = [
        "# MSG.6 — débit réel mesuré",
        "",
        f"> Mesure exécutée le **{date.today().isoformat()}** sur loopback.",
        "> Elle inclut le démarrage d'un processus, la génération de session, le chiffrement,",
        "> la signature ML-DSA-65, la fragmentation 4/7, le relais et l'écriture du fichier reçu.",
        "> Ce n'est **pas** un débit WAN et cela ne mesure pas le seul relais.",
        "",
        "## Conditions",
        "",
        f"- Binaire client : `{args.bin}`",
        f"- Relais : `{args.relay_bin}` sur `127.0.0.1`",
        f"- Taille utile par envoi : **{args.size} octets** ({args.size / 1024:.1f} Kio)",
        f"- Envois : **{args.runs}**",
        f"- SHA-256 du contenu source : `{payload_hash}`",
        "- Vérification : chaque fichier reçu a été relu et comparé octet à octet par SHA-256.",
        "",
        "## Mesures",
        "",
        "| Envoi | Durée (s) | Débit (Mio/s) |",
        "|---:|---:|---:|",
    ]
    for sample in samples:
        lines.append(f"| {sample['run']} | {float(sample['seconds']):.4f} | {float(sample['mib_s']):.3f} |")
    lines += [
        "",
        f"- Médiane : **{median:.3f} Mio/s**",
        f"- Moyenne : **{mean:.3f} Mio/s**",
        f"- Écart-type (population) : **{stdev:.3f} Mio/s**",
        f"- Minimum / maximum : **{min(rates):.3f} / {max(rates):.3f} Mio/s**",
        f"- Durée médiane d'un envoi : **{statistics.median(seconds):.4f} s**",
        "",
        "## Limites",
        "",
        "- Loopback uniquement ; aucun pare-feu, aucune latence WAN, aucune perte.",
        f"- Le débit inclut le coût fixe du processus et de l'identité, significatif pour {args.size / 1024:.1f} Kio.",
        "- Le relais plafonne une ligne à 64 Kio : un fichier plus gros est refusé avant envoi",
        "  (`enveloppe trop volumineuse`). Cette mesure ne couvre donc pas le transfert de gros fichiers.",
        "- Cette mesure **ne clôt pas MSG.6** : la preuve demandée est inter-hôtes, sur deux",
        "  machines physiques. Le loopback ne mesure ni pare-feu, ni WAN, ni perte réelle.",
        "- Les 10 fichiers sont conservés dans un `HOME` temporaire supprimé après la mesure.",
        "",
    ]
    report.write_text("\n".join(lines), encoding="utf-8")
    print(json.dumps({
        "runs": args.runs,
        "bytes": args.size,
        "median_mib_s": round(median, 3),
        "mean_mib_s": round(mean, 3),
        "stdev_mib_s": round(stdev, 3),
        "report": str(report),
        "sha256": payload_hash,
    }, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
