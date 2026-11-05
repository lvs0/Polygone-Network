#!/usr/bin/env python3
"""Mesure le coût réel de création d'identités Polygone, en HOME temporaires isolés."""
import argparse
import hashlib
import os
import platform
import statistics
import subprocess
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--samples", type=int, default=30)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/polygone")
    args = parser.parse_args()
    if not 5 <= args.samples <= 1000:
        parser.error("--samples doit être compris entre 5 et 1000")
    binary = args.binary.resolve()
    if not binary.is_file():
        parser.error(f"binaire introuvable : {binary} (construire --release d'abord)")

    times_ms, node_ids, sizes, modes = [], set(), [], set()
    with tempfile.TemporaryDirectory(prefix="polygone-sybil-cost-") as temp:
        for i in range(args.samples):
            home = Path(temp) / f"home-{i + 1:03d}"
            home.mkdir()
            env = os.environ.copy()
            env["HOME"] = str(home)
            started = time.perf_counter()
            result = subprocess.run(
                [str(binary), "id"], env=env, capture_output=True, text=True, timeout=30
            )
            elapsed = (time.perf_counter() - started) * 1000
            if result.returncode:
                raise RuntimeError(f"échantillon {i + 1}: {result.stderr.strip()}")
            node_id = result.stdout.strip().split()[0]
            if not node_id:
                raise RuntimeError(f"node_id absent à l'échantillon {i + 1}")
            node_ids.add(node_id)
            times_ms.append(elapsed)
            info = (home / ".polygone" / "identity.json").stat()
            sizes.append(info.st_size)
            modes.add(format(info.st_mode & 0o777, "03o"))

    ordered = sorted(times_ms)
    median = statistics.median(ordered)
    mean = statistics.mean(ordered)
    p95 = ordered[max(0, int(len(ordered) * 0.95 + 0.999999) - 1)]
    digest = hashlib.sha256(binary.read_bytes()).hexdigest()
    cpuinfo = Path("/proc/cpuinfo").read_text(errors="replace")
    cpu = next((line.split(":", 1)[1].strip() for line in cpuinfo.splitlines() if line.startswith("model name")), platform.machine())
    report = f"""# Mesure du coût de création d'identités Polygone

> Mesure locale datée du {time.strftime('%Y-%m-%dT%H:%M:%S%z')}. Ce rapport quantifie le coût de création sur cette machine ; il ne prouve pas une résistance Sybil.

## Protocole

- Binaire : {binary}
- SHA-256 du binaire : {digest}
- Machine : {cpu} ({platform.platform()})
- Échantillons : {args.samples} identités indépendantes, HOME temporaire neuf à chaque fois
- Action mesurée : lancement de polygone id, génération ML-KEM-1024 + ML-DSA-65, sérialisation et écriture de identity.json.
- Aucune identité existante n'est lue ou modifiée ; tous les HOME temporaires sont supprimés à la fin.

## Résultats

| Mesure | Résultat |
|---|---:|
| Durée médiane | {median:.2f} ms |
| Durée moyenne | {mean:.2f} ms |
| P95 | {p95:.2f} ms |
| Minimum / maximum | {ordered[0]:.2f} / {ordered[-1]:.2f} ms |
| Débit séquentiel observé | {60000 / mean:.1f} identités/min (hors concurrence) |
| Identités distinctes | {len(node_ids)}/{args.samples} |
| Taille identity.json | {min(sizes)}–{max(sizes)} octets |
| Modes de fichier observés | {', '.join(sorted(modes))} |

## Interprétation et limites

- Une identité coûte ici une génération de deux paires post-quantiques, un lancement de processus et une écriture locale ; ce n'est ni un prix, ni une preuve d'identité humaine.
- Les identités sont créables hors ligne et aucune autorité centrale, preuve de personne, dépôt de garantie ou quota global ne limite leur nombre dans ce protocole.
- Les quotas par identité limitent un pair donné, mais un adversaire peut créer plusieurs identités : Polygone ne doit pas être présenté comme Sybil-résistant sur cette seule base.
- La mesure est séquentielle sur un seul ThinkPad ; elle n'est pas un benchmark parallèle, multi-machine ou adversarial.
"""
    output = ROOT / "reports" / f"identity-cost-audit-{time.strftime('%Y-%m-%d')}.md"
    output.parent.mkdir(exist_ok=True)
    output.write_text(report)
    output.chmod(0o600)
    print(f"identités distinctes : {len(node_ids)}/{args.samples}")
    print(f"durée médiane {median:.2f} ms ; moyenne {mean:.2f} ms ; P95 {p95:.2f} ms")
    print(f"identity.json : {min(sizes)}–{max(sizes)} octets ; modes : {', '.join(sorted(modes))}")
    print(f"rapport : {output}")


if __name__ == "__main__":
    main()
