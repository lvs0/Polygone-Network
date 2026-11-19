#!/usr/bin/env node
// check-ci-honesty.mjs — CI.1 : la matrice ne promet pas ce qu'elle ne peut pas prouver.
//
// Vérifie trois choses, par lecture du texte (aucune dépendance) :
//
//   1. Aucun job BLOQUANT ne cible Windows. `daemon/src/resources/mod.rs`
//      contient un `compile_error!` : un job bloquant Windows est rouge par
//      construction et n'apprend rien.
//   2. Tout job qui cible Windows est explicitement autorisé à échouer
//      (`continue-on-error: true`), et son nom le dit.
//   3. La raison de l'exclusion est toujours vraie : le `compile_error!` existe
//      encore. S'il disparaît, ce script le signale au lieu de laisser un
//      commentaire périmé décider à la place du code.
//
// Ce script ne lance pas GitHub Actions et ne prétend pas le faire : il vérifie
// la structure du workflow. Le run distant reste la preuve de bout en bout.
//
// Sortie 0 si la matrice est honnête, 1 sinon, avec le job fautif nommé.

import { readFileSync, existsSync } from "node:fs";
import { join, dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const workflowPath = join(ROOT, ".github/workflows/ci.yml");
const failures = [];

if (!existsSync(workflowPath)) {
  console.error("CI : .github/workflows/ci.yml introuvable");
  process.exit(1);
}

const lines = readFileSync(workflowPath, "utf8").split("\n");

// Un job commence par exactement deux espaces puis `nom:`. On garde son bloc
// jusqu'au prochain job ou la fin du fichier.
const jobs = [];
let current = null;
for (const line of lines) {
  const start = line.match(/^ {2}([A-Za-z0-9_-]+):\s*$/);
  if (start) {
    current = { name: start[1], lines: [] };
    jobs.push(current);
    continue;
  }
  if (current) current.lines.push(line);
}

const isWindows = (job) => job.lines.some((line) => /windows-latest|windows-\d/.test(line));
const allowsFailure = (job) =>
  job.lines.some((line) => /^\s*continue-on-error:\s*true\s*$/.test(line));

if (jobs.length === 0) {
  failures.push("aucun job lisible : le format du workflow a changé");
}

for (const job of jobs) {
  if (!isWindows(job)) continue;
  if (!allowsFailure(job)) {
    failures.push(`job « ${job.name} » cible Windows sans continue-on-error: true`);
  }
  if (job.name === "test" || !/canary|canari|non bloquant/i.test(job.lines.join("\n"))) {
    failures.push(`job « ${job.name} » cible Windows : un job bloquant ne peut pas être vert`);
  }
}

const blocking = jobs.find((job) => job.name === "test");
if (!blocking) {
  failures.push("job bloquant « test » absent : le contrôle ne sait plus quoi vérifier");
} else if (!/ubuntu-latest/.test(blocking.lines.join("\n"))) {
  failures.push("job « test » ne cible plus Linux : la matrice a perdu sa base");
}

const resources = join(ROOT, "daemon/src/resources/mod.rs");
if (!existsSync(resources) || !readFileSync(resources, "utf8").includes("compile_error!")) {
  failures.push(
    "daemon/src/resources/mod.rs ne contient plus de compile_error! : Windows est peut-être supportable, la CI doit être reconsidérée",
  );
}

if (!readFileSync(workflowPath, "utf8").includes("check-ci-honesty.mjs")) {
  failures.push("le workflow ne lance pas ce contrôle : la porte ne se referme pas d'elle-même");
}

if (failures.length) {
  console.error(`${failures.length} écart(s) dans la matrice CI :`);
  for (const message of failures) console.error(`  - ${message}`);
  process.exit(1);
}

console.log(
  `matrice honnête : ${jobs.length} jobs, Windows hors du job bloquant et autorisé à échouer (compile_error! toujours présent)`,
);
