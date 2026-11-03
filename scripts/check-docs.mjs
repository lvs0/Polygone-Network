#!/usr/bin/env node
// check-docs.mjs — refuse un document qui contredit le code.
//
// Ce n'est pas un linter de prose. Il vérifie trois choses, et seulement trois :
//
//   1. Les CHIFFRES d'architecture (membres du workspace, LOC Rust, binaires)
//      sont recalculés depuis `Cargo.toml` et les sources, puis comparés à
//      `ARCHITECTURE.md`.
//   2. L'ARITHMÉTIQUE de `ETAT-REEL.md` : le score déclaré doit égaler le
//      nombre de lignes des tableaux §2/§3/§4.
//   3. Les PREUVES citées dans §2 pointent vers un fichier qui existe et une
//      ligne qui existe. Un document qui cite du code supprimé est un document
//      faux, même si sa prose est juste.
//
// Le nombre de tests ne peut pas être recalculé sans lancer la suite : il n'est
// donc PAS deviné. Passez `--tests N` (ou `POLYGONE_TEST_COUNT=N`) pour le
// vérifier ; sinon le script l'annonce comme non vérifié, au lieu de faire
// semblant.
//
// Sortie : 0 si tout concorde, 1 sinon. Chaque écart est nommé avec sa ligne.
// Zéro dépendance. Node >= 22.

import { readFileSync, readdirSync, statSync, existsSync } from "node:fs";
import { join, dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const failures = [];
const notes = [];

const read = (rel) => readFileSync(join(ROOT, rel), "utf8");
const lineOf = (text, offset) => text.slice(0, offset).split("\n").length;
const num = (s) => Number(String(s).replace(/[^0-9]/g, ""));

function fail(check, message) {
  failures.push({ check, message });
}

/* ───────────────────── 1. Chiffres d'architecture ───────────────────── */

function walk(dir, out = []) {
  let entries;
  try {
    entries = readdirSync(dir, { withFileTypes: true });
  } catch {
    return out;
  }
  for (const e of entries) {
    if (e.name === "target" || e.name === ".git" || e.name === "node_modules") continue;
    const full = join(dir, e.name);
    if (e.isDirectory()) walk(full, out);
    else out.push(full);
  }
  return out;
}

function workspaceFacts() {
  const rootToml = read("Cargo.toml");
  const membersBlock = rootToml.match(/members\s*=\s*\[([\s\S]*?)\]/);
  if (!membersBlock) throw new Error("Cargo.toml : bloc `members` introuvable");
  const members = [...membersBlock[1].matchAll(/"([^"]+)"/g)].map((m) => m[1]);

  // On ne compte QUE les membres du workspace. `salvage/` contient d'anciens
  // crates : les inclure gonflerait le chiffre et rendrait la mesure fausse
  // pour une raison que personne ne verrait dans le tableau.
  const perMember = members.map((member) => {
    const dir = join(ROOT, member);
    const files = walk(dir).filter((f) => f.endsWith(".rs"));
    const loc = files.reduce((sum, f) => sum + readFileSync(f, "utf8").split("\n").length - 1, 0);
    return { member, files: files.length, loc };
  });

  // Un binaire est soit déclaré `[[bin]]`, soit déduit de `src/main.rs` :
  // `crates/relay` n'a pas de section `[[bin]]` et produit pourtant
  // `polygone-relay`. Ne compter que les sections sous-estime le produit.
  const binaries = members.reduce((sum, member) => {
    const toml = join(ROOT, member, "Cargo.toml");
    if (!existsSync(toml)) return sum;
    const declared = [...readFileSync(toml, "utf8").matchAll(/^\[\[bin\]\]/gm)].length;
    const implicit = declared === 0 && existsSync(join(ROOT, member, "src/main.rs")) ? 1 : 0;
    return sum + declared + implicit;
  }, 0);

  return {
    members: members.length,
    memberNames: members,
    loc: perMember.reduce((s, m) => s + m.loc, 0),
    rustFiles: perMember.reduce((s, m) => s + m.files, 0),
    perMember,
    binaries,
  };
}

function checkArchitecture(facts) {
  const text = read("ARCHITECTURE.md");

  const declaredMembers = text.match(/(\d+)\s+(?:membres|members)\b/i);
  if (!declaredMembers) {
    fail("architecture.membres", "ARCHITECTURE.md ne déclare aucun nombre de membres");
  } else if (num(declaredMembers[1]) !== facts.members) {
    fail(
      "architecture.membres",
      `ARCHITECTURE.md déclare ${num(declaredMembers[1])} membres, Cargo.toml en liste ${facts.members}`,
    );
  }

  // Le tableau de synthèse porte la ligne « Total » : c'est elle qui fait foi.
  const lines = text.split("\n");
  const headerIndex = lines.findIndex((l) => /\|\s*Crate\s*\|/.test(l));
  const totalIndex = lines.findIndex((l) => /\|\s*\*\*Total\*\*\s*\|/.test(l));

  if (headerIndex < 0 || totalIndex < 0) {
    fail(
      "architecture.tableau",
      "ARCHITECTURE.md n'a plus de tableau Crate/…/Total : le contrôleur ne sait pas le lire, donc il refuse au lieu de passer",
    );
    return;
  }

  const header = lines[headerIndex].split("|").map((c) => c.trim()).filter(Boolean);
  const cells = lines[totalIndex]
    .split("|")
    .slice(1, -1)
    .map((c) => c.replace(/[*\s\u00a0]/g, ""));

  if (cells.length !== header.length) {
    fail("architecture.tableau", `ligne Total : ${cells.length} cellules pour ${header.length} colonnes`);
    return;
  }

  const column = (name) => {
    const i = header.findIndex((h) => new RegExp(name, "i").test(h));
    if (i < 0) fail("architecture.tableau", `colonne « ${name} » absente du tableau`);
    return i;
  };

  const checks = [
    { name: "fichiers", column: column("fichiers"), actual: facts.rustFiles, what: "fichiers Rust" },
    { name: "loc", column: column("LOC"), actual: facts.loc, what: "lignes de Rust" },
    { name: "binaires", column: column("binar"), actual: facts.binaries, what: "binaires" },
  ];

  for (const c of checks) {
    const declared = num(cells[c.column]);
    if (declared !== c.actual) {
      fail(
        `architecture.${c.name}`,
        `ARCHITECTURE.md déclare ${declared} ${c.what}, la mesure en donne ${c.actual}`,
      );
    }
  }

  // Les lignes doivent sommer à la ligne Total : un tableau qui se contredit
  // est aussi faux qu'un chiffre faux, et plus difficile à voir.
  const totalColumn = (name) => header.findIndex((h) => new RegExp(name, "i").test(h));
  const filesCol = totalColumn("fichiers");
  const testsCol = totalColumn("tests");
  const rows = lines
    .slice(headerIndex + 2, totalIndex)
    .filter((l) => l.startsWith("|"))
    .map((l) => l.split("|").slice(1, -1).map((c) => c.replace(/[*\s\u00a0]/g, "")));

  for (const [index, name] of [[filesCol, "fichiers"], [testsCol, "tests"]]) {
    if (index < 0) continue;
    const sum = rows.reduce((s, r) => s + (Number.isFinite(Number(r[index])) ? Number(r[index]) : 0), 0);
    const declared = num(cells[index]);
    if (sum !== 0 && sum !== declared) {
      fail(
        `architecture.somme.${name}`,
        `les lignes du tableau somment à ${sum} ${name}, la ligne Total déclare ${declared}`,
      );
    }
  }
}

/* ─────────────────────── 2. Arithmétique de ETAT-REEL ─────────────────────── */

function section(text, from, to) {
  const start = text.indexOf(from);
  if (start < 0) return null;
  const end = to ? text.indexOf(to, start) : text.length;
  return text.slice(start, end < 0 ? text.length : end);
}

function tableRows(block) {
  if (!block) return [];
  return block
    .split("\n")
    .filter((l) => /^\|\s*\d+\s*\|/.test(l))
    .map((l) => l);
}

function checkEtatReel() {
  const text = read("ETAT-REEL.md");

  const score = {
    vrai: num((text.match(/\*\*VRAI\*\*[^|]*\|\s*(\d+)/) ?? [])[1]),
    faux: num((text.match(/\*\*FAUX[^|]*\|\s*(\d+)/) ?? [])[1]),
    indetermine: num((text.match(/\*\*IND[EÉ]TERMIN[EÉ]\*\*[^|]*\|\s*(\d+)/) ?? [])[1]),
    total: num((text.match(/\*\*Total\*\*\s*\|\s*(\d+)/) ?? [])[1]),
  };

  // §2 et §3 sont des tableaux ; §4 est une liste numérotée. On ne force pas
  // une forme unique : on compte les éléments, pas les tuyaux.
  const listItems = (block) =>
    block ? block.split("\n").filter((l) => /^\d+\.\s+\S/.test(l)).length : 0;

  const sections = {
    vrai: tableRows(section(text, "## 2.", "## 3.")),
    faux: tableRows(section(text, "## 3.", "## 4.")),
    indetermine: Array.from({ length: listItems(section(text, "## 4.", "## 5.")) }),
  };

  for (const key of ["vrai", "faux", "indetermine"]) {
    if (!Number.isFinite(score[key])) {
      fail(`etat-reel.${key}`, `ETAT-REEL.md : score « ${key} » illisible`);
      continue;
    }
    if (sections[key].length !== score[key]) {
      fail(
        `etat-reel.${key}`,
        `ETAT-REEL.md déclare ${score[key]} capacités « ${key} » mais son tableau §${key === "vrai" ? 2 : key === "faux" ? 3 : 4} en contient ${sections[key].length}`,
      );
    }
  }

  const sum = score.vrai + score.faux + score.indetermine;
  if (Number.isFinite(score.total) && sum !== score.total) {
    fail(
      "etat-reel.somme",
      `ETAT-REEL.md : ${score.vrai} + ${score.faux} + ${score.indetermine} = ${sum}, mais le total déclaré est ${score.total}`,
    );
  }

  return { text, sections };
}

/* ────────────────── 3. Les preuves citées existent ────────────────── */

function resolveSource(ref) {
  const candidates = [
    join(ROOT, ref),
    join(ROOT, "crates/client/src", ref),
    join(ROOT, "daemon", ref),
  ];
  for (const c of candidates) if (existsSync(c)) return c;

  // Dernier recours : recherche par nom de fichier (les documents citent
  // souvent `net/mod.rs` sans le préfixe du crate).
  const base = ref.split("/").pop();
  const found = walk(ROOT).filter((f) => f.endsWith(`/${ref}`) || f.endsWith(`/${base}`));
  return found.length === 1 ? found[0] : null;
}

function checkProofs(text) {
  const block = section(text, "## 2.", "## 3.") ?? "";
  const seen = new Set();
  let checked = 0;

  for (const line of block.split("\n")) {
    if (!/^\|\s*\d+\s*\|/.test(line)) continue;
    const where = lineOf(block, block.indexOf(line));
    for (const m of line.matchAll(/`([A-Za-z0-9_./-]+\.(?:rs|toml|sh|js|html|md|json))(?::(\d+)(?:-(\d+))?)?`/g)) {
      const [, ref, from, to] = m;
      if (seen.has(ref)) continue;
      seen.add(ref);
      const path = resolveSource(ref);
      if (!path) {
        fail("preuves.fichier", `§2 ligne ${where} : \`${ref}\` est cité mais introuvable dans le dépôt`);
        continue;
      }
      checked += 1;
      const total = readFileSync(path, "utf8").split("\n").length;
      for (const n of [from, to]) {
        if (n && Number(n) > total) {
          fail(
            "preuves.ligne",
            `§2 ligne ${where} : \`${ref}:${n}\` dépasse la fin du fichier (${total} lignes)`,
          );
        }
      }
    }
  }
  notes.push(`preuves vérifiées : ${checked} fichiers cités existent et leurs lignes sont dans les bornes`);
}

/* ─────────────────── 4. Aucune revendication interdite ─────────────────── */

// `scope` limite la règle aux pages publiques : dans `ARCHITECTURE.md`, dire
// « zéro libp2p » ou décrire le monolithe abandonné est exact, alors que la
// même phrase sur une page produit est une revendication fausse.
const FORBIDDEN = [
  { pattern: /Five tabs/i, why: "la TUI n'a pas d'onglets (6 vues modales)" },
  { pattern: /Sparklines/i, why: "aucune sparkline n'est implémentée" },
  { pattern: /libp2p/i, why: "libp2p n'est plus une dépendance du workspace", scope: "web" },
  {
    pattern: /Kad DHT|GossipSub|AutoNAT|DCUtR/,
    why: "aucun de ces protocoles n'est présent dans le code",
    scope: "web",
  },
];

function checkClaims(files) {
  for (const rel of files) {
    if (!existsSync(join(ROOT, rel))) continue;
    const text = read(rel);
    for (const { pattern, why, scope } of FORBIDDEN) {
      if (scope === "web" && !rel.startsWith("web/")) continue;
      const m = text.match(pattern);
      if (m) {
        fail(
          "claim.interdite",
          `${rel}:${lineOf(text, m.index)} contient « ${m[0]} » — ${why}`,
        );
      }
    }
  }
}

/* ─────────────────────────── 5. Nombre de tests ─────────────────────────── */

function checkTestCount() {
  const flagIndex = process.argv.indexOf("--tests");
  const declared = flagIndex > 0 ? Number(process.argv[flagIndex + 1]) : Number(process.env.POLYGONE_TEST_COUNT);
  const text = read("ARCHITECTURE.md");

  // La valeur courante est celle de la ligne **Total** ; les parenthèses
  // historiques du genre « 109 tests (état du 2026-08-07) » ne comptent pas.
  const total = text.split("\n").find((l) => /\|\s*\*\*Total\*\*\s*\|/.test(l));
  const inDoc = total ? num(total.split("|").slice(1, -1).pop()) : NaN;

  if (!Number.isFinite(declared) || declared <= 0) {
    if (Number.isFinite(inDoc)) notes.push(`nombre de tests : ${inDoc} déclarés, NON VÉRIFIÉ (relancer avec --tests N)`);
    else notes.push("nombre de tests : NON VÉRIFIÉ (relancer avec --tests N, ou définir POLYGONE_TEST_COUNT)");
    return;
  }
  if (inDoc !== declared) {
    fail("architecture.tests", `ARCHITECTURE.md déclare ${inDoc} tests, la suite en exécute ${declared}`);
  }
}

/* ────────────────────────────────── MAIN ────────────────────────────────── */

const facts = workspaceFacts();
checkArchitecture(facts);
const { text: etat } = checkEtatReel();
checkProofs(etat);
checkClaims(["README.md", "ARCHITECTURE.md", "web/index.html", "web/landing.html"]);
checkTestCount();

console.log(`membres : ${facts.members} (${facts.memberNames.join(", ")})`);
console.log(`lignes de Rust : ${facts.loc} dans ${facts.rustFiles} fichiers`);
for (const m of facts.perMember) console.log(`  ${m.member.padEnd(22)} ${String(m.files).padStart(3)} fichiers  ${String(m.loc).padStart(6)} lignes`);
console.log(`binaires : ${facts.binaries}`);
for (const n of notes) console.log(`note : ${n}`);

if (failures.length) {
  console.error(`\n${failures.length} écart(s) documentaire(s) :`);
  for (const f of failures) console.error(`  [${f.check}] ${f.message}`);
  process.exit(1);
}

console.log("\ndocuments cohérents avec le code");
