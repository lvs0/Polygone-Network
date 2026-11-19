# Rapport d'audit Semgrep — Polygone-real

Date : 2026-10-14
Outil : Semgrep 1.180.0 (`--config=auto`)
Fichiers source scannés : 1054
Findings : 7

## Répartition par sévérité

- **WARNING** : 5
- **ERROR** : 1
- **INFO** : 1

## Findings détaillés

### .github/workflows/ci.yml

- **[WARNING]** ligne 18 — `yaml.github-actions.security.github-actions-mutable-action-tag.github-actions-mutable-action-tag`
  - GitHub Actions step uses a mutable tag or branch reference. Tags and branch names can be silently repointed by the action owner, enabling supply-chain attacks — as seen in the trivy-action and kics-github-action compromises. Pin the reference to a full 40-character commit SHA instead, e.g. `uses: actions/checkout@8ade135a41bc03ea155e62e844d188df1ea18608`.
- **[WARNING]** ligne 19 — `yaml.github-actions.security.github-actions-mutable-action-tag.github-actions-mutable-action-tag`
  - GitHub Actions step uses a mutable tag or branch reference. Tags and branch names can be silently repointed by the action owner, enabling supply-chain attacks — as seen in the trivy-action and kics-github-action compromises. Pin the reference to a full 40-character commit SHA instead, e.g. `uses: actions/checkout@8ade135a41bc03ea155e62e844d188df1ea18608`.
- **[WARNING]** ligne 40 — `yaml.github-actions.security.github-actions-mutable-action-tag.github-actions-mutable-action-tag`
  - GitHub Actions step uses a mutable tag or branch reference. Tags and branch names can be silently repointed by the action owner, enabling supply-chain attacks — as seen in the trivy-action and kics-github-action compromises. Pin the reference to a full 40-character commit SHA instead, e.g. `uses: actions/checkout@8ade135a41bc03ea155e62e844d188df1ea18608`.
- **[WARNING]** ligne 41 — `yaml.github-actions.security.github-actions-mutable-action-tag.github-actions-mutable-action-tag`
  - GitHub Actions step uses a mutable tag or branch reference. Tags and branch names can be silently repointed by the action owner, enabling supply-chain attacks — as seen in the trivy-action and kics-github-action compromises. Pin the reference to a full 40-character commit SHA instead, e.g. `uses: actions/checkout@8ade135a41bc03ea155e62e844d188df1ea18608`.

### docs/deployment/Dockerfile

- **[ERROR]** ligne 48 — `dockerfile.security.missing-user.missing-user`
  - By not specifying a USER, a program in the container may run as 'root'. This is a security hazard. If an attacker can control a process running as root, they may have control over the container. Ensure that the last USER in a Dockerfile is a USER other than 'root'.

### install.sh

- **[WARNING]** ligne 101 — `bash.lang.security.ifs-tampering.ifs-tampering`
  - The special variable IFS affects how splitting takes place when expanding unquoted variables. Don't set it globally. Prefer a dedicated utility such as 'cut' or 'awk' if you need to split input data. If you must use 'read', set IFS locally using e.g. 'IFS="," read -a my_array'.

### landing/server.js

- **[INFO]** ligne 4 — `javascript.express.security.audit.express-check-csurf-middleware-usage.express-check-csurf-middleware-usage`
  - A CSRF middleware was not detected in your express application. Ensure you are either using one such as `csurf` or `csrf` (see rule references) and/or you are properly doing CSRF validation in your routes with a token or cookies.

## Erreurs de parsing (bénignes)

- `.github/workflows/release.yml` — ['PartialParsing', [{'path': '.github/workflows/release.yml', 'start': {'line': 47, 'col': 27, 'offset': 1367}, 'end': {'line': 47, 'col': 42, 'offset': 1382}}]]
- `.github/workflows/release.yml` — ['PartialParsing', [{'path': '.github/workflows/release.yml', 'start': {'line': 47, 'col': 27, 'offset': 1367}, 'end': {'line': 47, 'col': 42, 'offset': 1382}}]]
- `scripts/forensic-zero-log.sh` — ['PartialParsing', [{'path': 'scripts/forensic-zero-log.sh', 'start': {'line': 15, 'col': 1, 'offset': 0}, 'end': {'line': 93, 'col': 8, 'offset': 3880}}]]
