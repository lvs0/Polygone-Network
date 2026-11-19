export const DOCUMENT_STATUSES = Object.freeze(["autorité", "daté", "brouillon", "à supprimer"]);

export function validateDocumentIndex({ indexText, markdownPaths, exists }) {
  const errors = [];
  const indexed = new Set();

  for (const [index, raw] of indexText.split("\n").entries()) {
    const line = raw.trim();
    if (!line.startsWith("| `docs/")) continue;

    const cells = line.split("|").slice(1, -1).map((cell) => cell.trim());
    const lineNumber = index + 1;
    if (cells.length !== 6) {
      errors.push(`docs/INDEX.md:${lineNumber} : ligne incomplète (${cells.length}/6 colonnes)`);
      continue;
    }

    const pathCell = cells[0];
    const path = pathCell.startsWith("`") && pathCell.endsWith("`")
      ? pathCell.slice(1, -1)
      : "";
    const [title, date, status, theme, justification] = cells.slice(1);

    if (!path || !path.startsWith("docs/") || !path.endsWith(".md")) {
      errors.push(`docs/INDEX.md:${lineNumber} : chemin Markdown invalide`);
      continue;
    }
    if (indexed.has(path)) errors.push(`docs/INDEX.md:${lineNumber} : ${path} apparaît plusieurs fois`);
    indexed.add(path);

    if (!exists(path)) errors.push(`docs/INDEX.md:${lineNumber} : ${path} ne correspond à aucun fichier`);
    if (!title) errors.push(`docs/INDEX.md:${lineNumber} : titre absent pour ${path}`);
    if (!date || !(date === "Non indiquée" || date.startsWith("Non indiquée (") || /^\d{4}-\d{2}-\d{2}\b/.test(date))) {
      errors.push(`docs/INDEX.md:${lineNumber} : date absente ou invalide pour ${path} (date ou « Non indiquée » requise)`);
    }
    if (!DOCUMENT_STATUSES.includes(status)) {
      errors.push(`docs/INDEX.md:${lineNumber} : statut absent ou invalide « ${status} » pour ${path}`);
    }
    if (!theme) errors.push(`docs/INDEX.md:${lineNumber} : thème absent pour ${path}`);
    if (!justification) errors.push(`docs/INDEX.md:${lineNumber} : justification absente pour ${path}`);
  }

  const actual = new Set(markdownPaths);
  for (const path of actual) {
    if (!indexed.has(path)) errors.push(`docs/INDEX.md : document Markdown oublié : ${path}`);
  }
  for (const path of indexed) {
    if (!actual.has(path)) errors.push(`docs/INDEX.md : entrée sans document Markdown : ${path}`);
  }

  return errors;
}
