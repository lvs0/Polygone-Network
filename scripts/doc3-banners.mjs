#!/usr/bin/env node
// doc3-banners.cjs — DOC.3 : poser un bandeau « Instantané daté » sur chaque
// document que `docs/INDEX.md` classe `daté`, sans réécrire son contenu.
//
// Le bandeau est dérivé de l'index : la date affichée est celle que l'index
// déclare (ou « date non indiquée »), et les liens pointent vers `ETAT-REEL.md`
// et `STATE.md` à la racine. Idempotent : un document qui contient déjà la
// mention exacte `Instantané daté` n'est pas touché.

import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { join, dirname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const index = readFileSync(join(ROOT, "docs/INDEX.md"), "utf8");

const cells = (line) =>
  line
    .split("|")
    .slice(1, -1)
    .map((cell) => cell.trim());

const rows = index
  .split("\n")
  .filter((line) => line.startsWith("|`") || line.startsWith("| `"))
  .map(cells)
  .filter((row) => row.length >= 6);

const touched = [];
const already = [];

for (const [file, , date, statut] of rows) {
  if (statut !== "daté") continue;
  const rel = file.replace(/`/g, "");
  const path = join(ROOT, rel);
  if (!existsSync(path)) {
    console.error(`absent : ${rel} (classé daté dans l'index)`);
    process.exitCode = 1;
    continue;
  }
  const text = readFileSync(path, "utf8");
  if (text.includes("Instantané daté")) {
    already.push(rel);
    continue;
  }

  const link = (name) =>
    relative(dirname(path), join(ROOT, name)).split("\\").join("/");
  const when = /^non indiquée/i.test(date)
    ? "Sa date de rédaction n'est pas indiquée"
    : `Rédigé le ${date}`;
  const banner =
    `> **Instantané daté — ne pas lire comme l'état courant.** ${when} ; ce ` +
    `document ne décrit pas l'état vérifié aujourd'hui, qui se lit dans ` +
    `[\`ETAT-REEL.md\`](${link("ETAT-REEL.md")}) et [\`STATE.md\`](${link("STATE.md")}).\n\n`;

  const lines = text.split("\n");
  const titleIndex = lines.findIndex((line) => line.startsWith("# "));
  const insertAt = titleIndex >= 0 ? titleIndex + 1 : 0;
  const before = lines.slice(0, insertAt).join("\n");
  const after = lines.slice(insertAt).join("\n");
  writeFileSync(path, `${before}\n\n${banner}${after.replace(/^\n+/, "")}`);
  touched.push(rel);
}

console.log(`bandeautés : ${touched.length}`);
for (const rel of touched) console.log(`  + ${rel}`);
console.log(`déjà bandeautés : ${already.length}`);
