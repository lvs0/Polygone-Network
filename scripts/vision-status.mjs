#!/usr/bin/env node
// Calcule l'avancement réel depuis docs/FOUNDER_VISION.md.
//
// Le registre est la seule source : une exigence validée est une ligne dont la
// colonne « État » vaut 6, et rien d'autre. Aucun chiffre n'est saisi à la main.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const registry = join(root, "docs", "FOUNDER_VISION.md");

const LABELS = {
  1: "non étudiée",
  2: "à concevoir",
  3: "à implémenter",
  4: "en cours",
  5: "implémentée mais non vérifiée",
  6: "testée et validée",
  7: "bloquée",
};

const lines = readFileSync(registry, "utf8").split("\n");
const rows = [];
const errors = [];

for (const [index, line] of lines.entries()) {
  if (!line.startsWith("| FV-")) continue;
  const cells = line.split("|").map((cell) => cell.trim());
  // cells[0] = "", cells[1] = id, cells[2] = exigence, cells[3] = état.
  const id = cells[1];
  const state = Number.parseInt(cells[3] ?? "", 10);
  if (!/^FV-[A-Z]+-\d{2}$/.test(id)) {
    errors.push(`ligne ${index + 1} : identifiant invalide « ${id} »`);
    continue;
  }
  if (!Number.isInteger(state) || state < 1 || state > 7) {
    errors.push(`ligne ${index + 1} (${id}) : état « ${cells[3]} » hors 1..7`);
    continue;
  }
  rows.push({ id, state });
}

if (rows.length < 10) {
  errors.push(`seulement ${rows.length} exigences lues : le registre est-il intact ?`);
}
if (errors.length > 0) {
  for (const error of errors) console.error(`ERREUR ${error}`);
  process.exit(1);
}

const counts = Object.fromEntries(Object.keys(LABELS).map((state) => [state, 0]));
for (const row of rows) counts[row.state] += 1;

const total = rows.length;
const validated = counts[6];
const blocked = counts[7];
const realistic = total - blocked;
const percent = (part, whole) => (whole === 0 ? "0.0" : ((part / whole) * 100).toFixed(1));

console.log(`Registre        : ${total} exigences`);
for (const state of Object.keys(LABELS)) {
  console.log(`  ${state} ${LABELS[state].padEnd(34)} ${String(counts[state]).padStart(3)}`);
}
console.log(`Validées (6)    : ${validated}/${total} = ${percent(validated, total)} %`);
console.log(`Réalisables     : ${realistic}/${total} (hors ${blocked} bloquées)`);
console.log(`Avancement réel : ${validated}/${realistic} = ${percent(validated, realistic)} %`);

if (process.argv.includes("--json")) {
  console.log(JSON.stringify({ total, counts, validated, blocked, realistic }, null, 2));
}
