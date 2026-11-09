#!/usr/bin/env bash
# Reprise de mission : imprime l'état réel du dépôt canonique et la prochaine
# action, sans réexploration. À lancer en début de session.
#
#   ./scripts/mission-resume.sh
#
# Ce script ne modifie rien. Il dit où on est, ce qui est prouvé, et ce qui
# bloque une publication.
set -Eeuo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "════════════════════════════════════════════════════════════════════"
echo " POLYGONE — reprise de mission"
echo "════════════════════════════════════════════════════════════════════"
printf 'dépôt      : %s\n' "$ROOT"
printf 'branche    : %s\n' "$(git rev-parse --abbrev-ref HEAD)"
printf 'HEAD       : %s\n' "$(git log -1 --format='%h %s')"
printf 'arbre      : %s entrée(s) non commitée(s)\n' "$(git status --porcelain | wc -l)"
printf 'disque     : %s libre\n' "$(df -h "$ROOT" | awk 'NR==2{print $4}')"
echo
echo "── preuves présentes ──────────────────────────────────────────────"
if [[ -d docs/proofs ]]; then
  ls -1 docs/proofs | sed 's/^/  · /'
else
  echo "  (aucune : docs/proofs/ absent)"
fi
echo
echo "── prochaine action ───────────────────────────────────────────────"
if [[ -f project/STATUS.md ]]; then
  awk '/^## 3\. Prochaine action/{flag=1;next} /^## 4\./{flag=0} flag && NF' project/STATUS.md
else
  echo "  project/STATUS.md absent — reconstituer l'état avant de continuer"
fi
echo
echo "── ce qui bloque une publication ──────────────────────────────────"
if [[ -f project/STATUS.md ]]; then
  awk '/^## 5\. Ce qui interdit/{flag=1;next} /^## 6\./{flag=0} flag && NF' project/STATUS.md
fi
echo
echo "── porte de qualité ───────────────────────────────────────────────"
echo "  make check     fmt + clippy + tests + documentation"
echo "  make smoke     parcours réel à trois processus"
echo "════════════════════════════════════════════════════════════════════"
