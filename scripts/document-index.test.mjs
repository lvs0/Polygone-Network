import test from "node:test";
import assert from "node:assert/strict";
import { validateDocumentIndex } from "./document-index.mjs";

const path = "docs/example.md";
const row = (date = "2026-11-05", status = "brouillon") =>
  `| \`docs/example.md\` | Exemple | ${date} | ${status} | Test | Justification |\n`;
const validate = (indexText) => validateDocumentIndex({
  indexText,
  markdownPaths: [path],
  exists: (candidate) => candidate === path,
});

test("accepte une entrée complète avec date et statut valides", () => {
  assert.deepEqual(validate(row()), []);
});

test("accepte explicitement une date non indiquée", () => {
  assert.deepEqual(validate(row("Non indiquée")), []);
});

test("refuse une date absente", () => {
  assert.match(validate(row("")).join("\n"), /date absente ou invalide/);
});

test("refuse un statut inconnu ou absent", () => {
  assert.match(validate(row("2026-11-05", "")).join("\n"), /statut absent ou invalide/);
  assert.match(validate(row("2026-11-05", "presque fait")).join("\n"), /statut absent ou invalide/);
});

test("refuse une ligne incomplète", () => {
  assert.match(validate("| `docs/example.md` | Exemple | 2026-11-05 |\n").join("\n"), /ligne incomplète/);
});

test("refuse un document oublié et un doublon", () => {
  const errors = validate(row() + row());
  assert.match(errors.join("\n"), /apparaît plusieurs fois/);
});
