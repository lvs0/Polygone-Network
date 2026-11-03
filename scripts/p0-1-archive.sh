#!/usr/bin/env bash
# P0.1 — Archive restaurable du dépôt canonique Polygone-real.
#
# Objectif : pouvoir restaurer l'état EXACT (y compris les 65 fichiers non
# commités et l'historique .git) si une manipulation tourne mal.
#
# Exclus uniquement ce qui est régénérable par l'outillage :
#   target/        (11 Go, reconstruit par cargo)
#   node_modules/  (reconstruit par npm/pnpm)
# Tout le reste est inclus, y compris .git et les fichiers non suivis.
#
# Sortie : une archive + un rapport de vérification qui prouve la restauration
# (listing + extraction réelle dans un répertoire jetable + cmp sur 3 fichiers
# critiques + relecture du HEAD git depuis l'extraction).
set -uo pipefail

SRC=/home/lvs0/Projects/Polygone-real
STAMP=$(date +%Y%m%d-%H%M%S)
DEST=/home/lvs0/Backups
ARCHIVE="$DEST/polygone-real-$STAMP.tar.gz"
SUMS="$ARCHIVE.sha256"
VERIFY="/tmp/p01-verify-$STAMP"
REPORT="$DEST/polygone-real-$STAMP.report.md"

mkdir -p "$DEST"
FAIL=0

echo "[1/6] Archive en cours → $ARCHIVE"
# stderr de tar capturé, JAMAIS jeté : un avertissement « file changed as we
# read it » est exactement ce qu'il faut voir, pas cacher.
TAR_ERR=$(mktemp)
tar -czf "$ARCHIVE" \
    --exclude='./target' \
    --exclude='*/node_modules' \
    -C "$SRC" . 2>"$TAR_ERR"
TAR_RC=$?
echo "      tar exit=$TAR_RC"
TAR_WARN=$(cat "$TAR_ERR")
[ -n "$TAR_WARN" ] && { echo "      tar stderr :"; sed 's/^/        /' "$TAR_ERR"; }
# Un tar non-nul invalide la preuve, même si l'extraction réussit : on ne
# déclare pas PASS sur une archive dont la création a signalé un problème.
[ "$TAR_RC" -eq 0 ] || FAIL=1

echo "[2/6] Empreinte SHA-256"
sha256sum "$ARCHIVE" > "$SUMS"
SHA=$(cut -d' ' -f1 "$SUMS")
SIZE=$(du -h "$ARCHIVE" | cut -f1)

echo "[3/6] Listing de contrôle"
LISTED=$(tar -tzf "$ARCHIVE" 2>/dev/null | wc -l)
echo "      $LISTED entrées listées"
[ "$LISTED" -gt 500 ] || { echo "      ÉCHEC : trop peu d'entrées"; FAIL=1; }

echo "[4/6] Extraction réelle dans $VERIFY"
rm -rf "$VERIFY"; mkdir -p "$VERIFY"
tar -xzf "$ARCHIVE" -C "$VERIFY" 2>/dev/null
EX_RC=$?
echo "      extraction exit=$EX_RC"
[ "$EX_RC" -eq 0 ] || FAIL=1

echo "[5/6] cmp sur 3 fichiers critiques"
CMP_ROWS=""
for f in Cargo.toml crates/core/src/lib.rs daemon/src/socket.rs; do
    if cmp -s "$SRC/$f" "$VERIFY/$f"; then
        CMP_ROWS+="| \`$f\` | identique |"$'\n'
    else
        CMP_ROWS+="| \`$f\` | **DIFFÉRENT** |"$'\n'
        FAIL=1
    fi
done

echo "[6/6] Historique git restauré"
HEAD_SRC=$(git -C "$SRC" rev-parse HEAD 2>/dev/null || echo "inconnu")
HEAD_EX=$(git -C "$VERIFY" rev-parse HEAD 2>/dev/null || echo "inconnu")
[ "$HEAD_SRC" = "$HEAD_EX" ] && [ "$HEAD_SRC" != "inconnu" ] || FAIL=1
DIRTY_SRC=$(git -C "$SRC" status --porcelain 2>/dev/null | wc -l)
DIRTY_EX=$(git -C "$VERIFY" status --porcelain 2>/dev/null | wc -l)
[ "$DIRTY_SRC" = "$DIRTY_EX" ] || FAIL=1

VERDICT=$([ "$FAIL" -eq 0 ] && echo "PASS" || echo "FAIL")

cat > "$REPORT" <<EOF
# P0.1 — Archive restaurable · rapport

| Champ | Valeur |
|---|---|
| Date | $STAMP |
| Source | \`$SRC\` |
| Archive | \`$ARCHIVE\` |
| Taille | $SIZE |
| SHA-256 | \`$SHA\` |
| Entrées listées | $LISTED |
| tar exit | $TAR_RC |
| tar stderr | ${TAR_WARN:-aucun} |
| extraction exit | $EX_RC |
| HEAD source | \`$HEAD_SRC\` |
| HEAD extrait | \`$HEAD_EX\` |
| Fichiers modifiés/non suivis (source) | $DIRTY_SRC |
| Fichiers modifiés/non suivis (extrait) | $DIRTY_EX |

## cmp sur 3 fichiers critiques

| Fichier | Résultat |
|---|---|
$CMP_ROWS
## Exclusions assumées

\`target/\` (11 Go, régénérable par \`cargo\`) et tout \`node_modules/\`
(régénérable par npm). Aucune autre exclusion : \`.git\`, les fichiers suivis
et les fichiers non suivis sont dans l'archive.

## Restauration

\`\`\`bash
sha256sum -c $SUMS
tar -xzf $ARCHIVE -C /chemin/de/restauration
\`\`\`

## Verdict

**$VERDICT**
EOF

echo
echo "VERDICT=$VERDICT"
cat "$REPORT"

# Le répertoire jetable est conservé : il est la preuve de l'extraction.
# Le supprimer rendrait le rapport invérifiable.
echo "PREUVE=$VERIFY"
exit "$FAIL"
