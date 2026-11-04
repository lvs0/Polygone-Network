#!/usr/bin/env node
// orchestrateur.mjs — boucle de travail Polygone.
//
// Ce n'est pas un cron qui rapporte : c'est une boucle qui
//   découvre → choisit → dispatche → vérifie → commite → écrit l'état
// et qui refuse de prétendre avoir réussi sans sortie de commande.
//
// Modes :
//   node orchestrateur.mjs --status
//   node orchestrateur.mjs --discover
//   node orchestrateur.mjs --once  [--execute] [--commit] [--task P1.4]
//   node orchestrateur.mjs --loop  --hours 4 [--execute] [--commit] [--push]
//
// Sans `--execute`, rien n'est lancé : le brief est écrit, et c'est tout.
// Sans `--commit`, aucun commit n'est créé.
//
// Zéro dépendance. Node >= 22.

import { spawn } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync, appendFileSync, readdirSync, statSync } from "node:fs";
import { dirname, join, relative, resolve, isAbsolute } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const CONFIG_PATH = process.env.POLYGONE_DISPATCH ?? join(HERE, "dispatch.json");
const CONFIG = readConfig();
const REPO = resolve(HERE, CONFIG.repo ?? "../../..");
const STATE_DIR = join(REPO, ".orchestrator");
const STATE_PATH = join(STATE_DIR, "state.json");
const LOG_DIR = join(REPO, "logs");
const BRIEF_DIR = join(STATE_DIR, "briefs");
const STOP_FILE = join(REPO, "STOP");
const JOURNAL = join(LOG_DIR, `orchestrateur-${today()}.log`);

const args = process.argv.slice(2);
const has = (flag) => args.includes(flag);
const valueOf = (flag, fallback = null) => {
  const i = args.indexOf(flag);
  return i >= 0 && args[i + 1] ? args[i + 1] : fallback;
};

function readConfig() {
  const path = process.env.POLYGONE_DISPATCH ?? join(HERE, "dispatch.json");
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch (err) {
    console.error(`orchestrateur: configuration illisible (${path}) — ${err.message}`);
    process.exit(2);
  }
}

function today() {
  return new Date().toISOString().slice(0, 10);
}
function stamp() {
  return new Date().toISOString().replace("T", " ").slice(0, 19);
}
function log(line) {
  mkdirSync(LOG_DIR, { recursive: true });
  appendFileSync(JOURNAL, `[${stamp()}] ${line}\n`);
  console.log(`[${stamp()}] ${line}`);
}

function readJson(path, fallback) {
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch {
    return fallback;
  }
}

function writeJson(path, value) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, JSON.stringify(value, null, 2) + "\n");
}

function loadState() {
  return readJson(STATE_PATH, {
    createdAt: new Date().toISOString(),
    lastRunAt: null,
    current: null,
    completed: [],
    blocked: [],
    session: { tasks: 0, commits: 0, failedChecks: 0 },
  });
}

function saveState(state) {
  state.lastRunAt = new Date().toISOString();
  writeJson(STATE_PATH, state);
}

/* ────────────────────────────── 1. DÉCOUVRIR ────────────────────────────── */

const TODO_PATTERN = /\b(TODO|FIXME|HACK|XXX)\b[:\s]*(.*)/i;
const SKIP_DIRS = new Set(["target", "node_modules", ".git", "dist", "build", ".orchestrator", "logs"]);

function walk(dir, out = [], limit = 4000) {
  if (out.length >= limit) return out;
  let entries;
  try {
    entries = readdirSync(dir, { withFileTypes: true });
  } catch {
    return out;
  }
  for (const entry of entries) {
    if (out.length >= limit) break;
    if (entry.name.startsWith(".") && entry.name !== ".github") continue;
    if (SKIP_DIRS.has(entry.name)) continue;
    const full = join(dir, entry.name);
    if (entry.isDirectory()) walk(full, out, limit);
    else if (/\.(rs|mjs|js|ts|md|toml|sh|py)$/.test(entry.name)) out.push(full);
  }
  return out;
}

/** Découverte : TODO/FIXME réels + tests ignorés. Ce n'est pas une liste de vœux. */
function discover() {
  const files = walk(REPO);
  const todos = [];
  const ignoredTests = [];
  for (const file of files) {
    let text;
    try {
      text = readFileSync(file, "utf8");
    } catch {
      continue;
    }
    const rel = relative(REPO, file);
    if (/\{\/\*|#\[ignore\]|\.skip\(|it\.todo\(/.test(text) && /\.(rs|mjs|js|ts)$/.test(file)) {
      ignoredTests.push(rel);
    }
    if (!/\.(rs|mjs|js|ts|sh|py|toml)$/.test(file)) continue;
    const lines = text.split("\n");
    for (let i = 0; i < lines.length; i += 1) {
      const match = lines[i].match(TODO_PATTERN);
      if (match) todos.push({ file: rel, line: i + 1, text: (match[2] || "").trim().slice(0, 160) });
    }
  }
  return { todos, ignoredTests, scanned: files.length };
}

/* ────────────────────────────── 2. CHOISIR ──────────────────────────────── */

/**
 * Lit le backlog. Un backlog illisible ne doit pas ressembler à un backlog vide :
 * silencieusement, la boucle conclurait « rien à faire » et s'arrêterait.
 */
function readBacklogData() {
  const path = join(HERE, "backlog.json");
  let raw;
  try { raw = readFileSync(path, "utf8"); }
  catch (err) { throw new Error("backlog illisible (" + path + ") — " + err.message); }
  let data;
  try { data = JSON.parse(raw); }
  catch (err) { throw new Error("backlog.json invalide — " + err.message); }
  if (!Array.isArray(data.tasks)) throw new Error("backlog.json : tasks absent ou non tableau");
  if (data.completed !== undefined && !Array.isArray(data.completed)) throw new Error("backlog.json : completed non tableau");
  const seen = new Set();
  for (const t of data.tasks) {
    if (!t || typeof t.id !== "string" || !t.id) throw new Error("une tâche n'a pas d'identifiant");
    if (seen.has(t.id)) throw new Error("identifiant en double : " + t.id);
    seen.add(t.id);
    if (!t.phase) throw new Error(t.id + " n'a pas de phase");
    if (!Number.isFinite(Number(t.priority))) throw new Error(t.id + " n'a pas de priorité numérique");
  }
  for (const item of data.completed ?? []) {
    if (!item || typeof item.id !== "string" || !item.id) throw new Error("entrée completed sans identifiant");
    if (seen.has(item.id)) throw new Error(item.id + " est à la fois ouvert et terminé");
    seen.add(item.id);
  }
  return data;
}
function readBacklog() { return readBacklogData().tasks; }
function completedBacklogIds() { return (readBacklogData().completed ?? []).map((item) => item.id); }

function blockedError(message) {
  const err = new Error(message);
  err.code = "BLOCKED";
  return err;
}

/**
 * Dépendances non satisfaites, en clair.
 * Une forme inattendue BLOQUE : se tromper en bloquant est réparable, se tromper
 * en débloquant ne l'est pas.
 */
function openBlockers(task, done) {
  const b = task.blocked_by;
  if (b === undefined || b === null) return [];
  if (typeof b === "string") return [`dépendance non structurée : ${b}`];
  const out = [];
  for (const id of b.tasks ?? []) {
    if (!done.has(id)) out.push(`tâche ${id} non terminée`);
  }
  for (const d of b.decisions ?? []) out.push(`décision humaine ${d}`);
  for (const key of Object.keys(b)) {
    if (key !== "tasks" && key !== "decisions") out.push(`champ inconnu blocked_by.${key}`);
  }
  return out;
}

/** Le backlog déclaré est la seule source de tâches ; la découverte l'alimente. */
function chooseTask(state) {
  const forced = valueOf("--task");
  const backlog = readBacklog();
  const done = new Set([...(state.completed ?? []), ...completedBacklogIds()]);
  const failed = new Set((state.blocked ?? []).map((b) => b.id));

  if (forced) {
    const task = backlog.find((t) => t.id === forced);
    if (!task) throw new Error(`tâche inconnue : ${forced} (${backlog.length} tâches déclarées)`);
    const blockers = openBlockers(task, done);
    if (blockers.length) throw blockedError(`tâche ${forced} BLOQUÉE — ${blockers.join(" ; ")}`);
    if (failed.has(task.id)) throw blockedError(`tâche ${forced} BLOQUÉE — échec de vérification précédent, à diagnostiquer`);
    return task;
  }

  const order = backlog
    .filter((t) => !done.has(t.id) && !failed.has(t.id) && openBlockers(t, done).length === 0)
    .sort((a, b) => a.phase.localeCompare(b.phase) || Number(a.priority) - Number(b.priority));

  return order[0] ?? null;
}

/** Répartition du backlog : ouverte / bloquée (avec motifs) / terminée. */
function backlogStatus(state) {
  const backlog = readBacklog();
  const done = new Set([...(state.completed ?? []), ...completedBacklogIds()]);
  const failed = new Set((state.blocked ?? []).map((b) => b.id));
  const open = [];
  const blockedTasks = [];
  const completed = new Set(done);
  for (const t of backlog) {
    if (done.has(t.id)) {
      completed.add(t.id);
      continue;
    }
    const blockers = openBlockers(t, done);
    if (failed.has(t.id)) blockers.push("échec de vérification précédent, à diagnostiquer");
    if (blockers.length) blockedTasks.push({ id: t.id, phase: t.phase, blockers });
    else open.push({ id: t.id, phase: t.phase, priority: Number(t.priority) });
  }
  open.sort((a, b) => a.phase.localeCompare(b.phase) || a.priority - b.priority);
  return {
    total: new Set([...backlog.map((t) => t.id), ...done]).size,
    open: open.length,
    blocked: blockedTasks.length,
    completed: completed.size,
    next: open[0]?.id ?? null,
    openTasks: open.map((t) => t.id),
    blockedTasks,
    completedTasks: [...completed],
  };
}

/* ────────────────────────────── 3. DISPATCHER ───────────────────────────── */

function briefOf(task) {
  return `# Brief — ${task.id} ${task.title}

Tu es un worker de l'orchestrateur Polygone. Tu n'as pas de conversation
précédente : tout ce dont tu as besoin est ci-dessous.

## Contexte à lire AVANT d'écrire
- \`VISION.md\` — ce qui fait autorité
- \`ETAT-REEL.md\` — ce qui est vrai sur le code
- \`docs/atlas/agent/ORDRE.md\` — les interdits (surtout §1)

## Tâche
${task.title}

${task.detail ?? ""}

## Sortie attendue (vérifiable)
${task.proof ?? "Une commande exécutée, sa sortie citée, un commit."}

## Fichiers probables
${(task.files ?? []).map((f) => `- ${f}`).join("\n") || "- à déterminer par lecture du code"}

## Interdits
- Ne pas modifier un test pour le faire passer.
- Ne pas toucher à un secret, un .env, \`~/.polygone\`.
- Ne pas inventer une fonctionnalité hors périmètre.
- Ne pas déclarer « fait » sans avoir exécuté la preuve.

## Rapport final
Liste exactement les fichiers que tu as édités, la commande exécutée, et sa
sortie. Si tu n'as pas pu prouver, écris BLOQUÉ et la cause.
`;
}

function run(command, { cwd = REPO, timeoutMs = CONFIG.taskTimeoutMs ?? 1_800_000, quiet = false } = {}) {
  return new Promise((resolve) => {
    const [bin, ...rest] = command;
    const child = spawn(bin, rest, { cwd, env: process.env, stdio: ["ignore", "pipe", "pipe"] });
    let stdout = "";
    let stderr = "";
    const timer = setTimeout(() => {
      child.kill("SIGTERM");
      setTimeout(() => child.kill("SIGKILL"), 5_000);
    }, timeoutMs);

    child.stdout.on("data", (d) => {
      stdout += d;
      if (!quiet) process.stdout.write(d);
    });
    child.stderr.on("data", (d) => {
      stderr += d;
      if (!quiet) process.stderr.write(d);
    });
    child.on("error", (err) => {
      clearTimeout(timer);
      resolve({ code: -1, stdout, stderr: `${stderr}${err.message}` });
    });
    child.on("close", (code) => {
      clearTimeout(timer);
      resolve({ code: code ?? -1, stdout, stderr });
    });
  });
}

async function dispatch(task) {
  mkdirSync(BRIEF_DIR, { recursive: true });
  const briefPath = join(BRIEF_DIR, `${task.id.replace(/[^\w.-]/g, "_")}.md`);
  writeFileSync(briefPath, briefOf(task));

  const template = CONFIG.agentCommand;
  if (!Array.isArray(template) || template.length === 0) {
    log(`dispatch ${task.id}: aucun agentCommand configuré — brief écrit, rien lancé`);
    return { executed: false, briefPath, code: null, stdout: "", stderr: "" };
  }

  const command = template.map((part) =>
    part.replaceAll("{prompt}", briefPath).replaceAll("{repo}", REPO).replaceAll("{task}", task.id),
  );
  log(`dispatch ${task.id}: ${command.join(" ")}`);
  const result = await run(command, { timeoutMs: CONFIG.taskTimeoutMs ?? 1_800_000 });
  log(`dispatch ${task.id}: code=${result.code}`);
  return { executed: true, briefPath, ...result };
}

/* ────────────────────────────── 4. VÉRIFIER ─────────────────────────────── */

async function verify() {
  const checks = CONFIG.checks ?? [];
  const results = [];
  for (const check of checks) {
    if (Array.isArray(check.optional) && check.optional.every((p) => !existsSync(join(REPO, p)))) {
      results.push({ name: check.name, skipped: true, reason: "prérequis absent" });
      continue;
    }
    log(`check ${check.name}: ${check.command.join(" ")}`);
    const result = await run(check.command, { quiet: true });
    results.push({ name: check.name, code: result.code, tail: (result.stdout + result.stderr).slice(-1500) });
    log(`check ${check.name}: ${result.code === 0 ? "OK" : `ÉCHEC (${result.code})`}`);
    if (result.code !== 0) return { ok: false, results };
  }
  return { ok: true, results };
}

/* ────────────────────────────── 5. COMMITER ─────────────────────────────── */

async function git(args, opts) {
  return run(["git", ...args], { quiet: true, ...opts });
}

async function commitIfNeeded(task, verification) {
  if (has("--push") && CONFIG.allowPush !== true) log("push refusé : allowPush non activé");
  const allowed = (task.files ?? []).filter((p) =>
    typeof p === "string" && p.length > 0 && !isAbsolute(p) &&
    !p.split(/[\\/]/).includes("..") &&
    !/(^|[\\/])(?:\.git|target|node_modules|\.orchestrator|logs)(?:[\\/]|$)/.test(p) &&
    !/(^|[\\/])\.env(?:\.|$)/.test(p)
  );
  if (!allowed.length) { log("commit refusé : aucun périmètre task.files sûr"); return null; }
  const inScope = (file) => allowed.some((p) => {
    const base = p.replace(/[\\/]+$/, "");
    return file === base || file.startsWith(base + "/");
  });
  const staged0 = (await git(["diff","--cached","--name-only","-z"])).stdout.split("\0").filter(Boolean);
  if (staged0.some((f) => !inScope(f))) { log("commit refusé : staging préexistant hors périmètre"); return null; }
  const records = (await git(["status","--porcelain","-z","--untracked-files=all"])).stdout.split("\0").filter(Boolean);
  if (records.some((r) => /^(?:R|C)/.test(r.slice(0,2)))) { log("commit refusé : renommage/copie à revoir"); return null; }
  const changed = records.map((r) => r.slice(3)).filter(Boolean);
  const toStage = changed.filter(inScope);
  if (toStage.length) {
    const add = await git(["add","--",...toStage]);
    if (add.code !== 0) { log("staging refusé : " + add.stderr.slice(-250)); return null; }
  }
  const staged = (await git(["diff","--cached","--name-only","-z"])).stdout.split("\0").filter(Boolean);
  if (!staged.length || staged.some((f) => !inScope(f))) { log("commit refusé : aucun changement sûr ou fichier hors périmètre"); return null; }
  const message = ((CONFIG.commitPrefix ?? "polygone") + ": " + task.title + " — " + (task.proof ?? "vérifié")).slice(0,180);
  const result = await git(["commit","-m",message]);
  if (result.code !== 0) { log("commit échoué : " + result.stderr.slice(-250)); return null; }
  const head = await git(["log","-1","--format=%h %s"]);
  log("commit : " + head.stdout.trim());
  if (has("--push") && CONFIG.allowPush === true) {
    const branch = (await git(["rev-parse","--abbrev-ref","HEAD"])).stdout.trim();
    const push = await git(["push","origin",branch]);
    log("push " + branch + " : " + (push.code === 0 ? "OK" : "ÉCHEC — " + push.stderr.slice(-250)));
  }
  return head.stdout.trim();
}

/* ────────────────────────────── 6. ÉTAT ─────────────────────────────────── */

function writeSessionReport(task, verification, commit) {
  const path = join(LOG_DIR, `${new Date().toISOString().slice(0, 16).replace(/[:T]/g, "-")}-${task.id.replace(/[^\w.-]/g, "_")}.log`);
  mkdirSync(LOG_DIR, { recursive: true });
  writeFileSync(
    path,
    [
      `## Tâche: ${task.id} ${task.title}`,
      `## Verdict: ${verification.ok ? (commit ? "FAIT" : "FAIT (non commité)") : "BLOQUÉ"}`,
      `## Preuve: ${task.proof ?? "—"}`,
      ...verification.results.map((r) =>
        r.skipped ? `## Check ${r.name}: ignoré (${r.reason})` : `## Check ${r.name}: ${r.code === 0 ? "OK" : "ÉCHEC"}`,
      ),
      `## Commit: ${commit ?? "aucun"}`,
      `## Reste: ${verification.ok ? "tâche suivante du backlog" : "diagnostiquer l'échec, ne pas contourner"}`,
      "",
    ].join("\n"),
  );
  return path;
}

/* ────────────────────────────── BOUCLE ─────────────────────────────────── */

async function once(state) {
  if (existsSync(STOP_FILE)) {
    log("STOP présent — arrêt propre demandé");
    return { stopped: true };
  }

  const task = chooseTask(state);
  if (!task) {
    log("backlog vide pour les phases ouvertes — rien à faire");
    return { empty: true };
  }

  state.current = task.id;
  state.session.tasks += 1;
  saveState(state);
  log(`tâche: ${task.id} ${task.title} (phase ${task.phase})`);

  if (!has("--execute")) {
    const brief = briefOf(task);
    mkdirSync(BRIEF_DIR, { recursive: true });
    const p = join(BRIEF_DIR, `${task.id.replace(/[^\w.-]/g, "_")}.md`);
    writeFileSync(p, brief);
    log(`dry-run: brief écrit dans ${relative(REPO, p)} — relancer avec --execute`);
    return { dryRun: true, task };
  }

  const result = await dispatch(task);
  if (!result.executed) {
    state.current = null;
    saveState(state);
    log("tâche " + task.id + " NON EXÉCUTÉE : aucun worker configuré ; brief conservé, tâche non cochée");
    return { notExecuted: true, task, briefPath: result.briefPath };
  }
  if (result.code !== 0) {
    state.blocked.push({ id: task.id, at: new Date().toISOString(), reason: `agent code=${result.code}` });
    state.session.failedChecks += 1;
    saveState(state);
    log(`tâche ${task.id}: BLOQUÉE par le worker (code ${result.code})`);
    return { blocked: true, task };
  }

  const verification = await verify();
  if (!verification.ok) {
    state.session.failedChecks += 1;
    state.blocked.push({ id: task.id, at: new Date().toISOString(), reason: "vérification échouée" });
    saveState(state);
    writeSessionReport(task, verification, null);
    log(`tâche ${task.id}: vérification ÉCHOUÉE — pas de commit`);
    return { failed: true, task };
  }

  const commit = has("--commit") ? await commitIfNeeded(task, verification) : null;
  writeSessionReport(task, verification, commit);
  state.completed.push(task.id);
  state.current = null;
  if (commit) state.session.commits += 1;
  saveState(state);
  log(`tâche ${task.id}: FAIT${commit ? ` (${commit})` : " (non commité)"}`);
  return { done: true, task, commit };
}

async function loop(state) {
  const hours = Number(valueOf("--hours", "1")) || 1;
  const deadline = Date.now() + hours * 3_600_000;
  let idle = 0;

  while (Date.now() < deadline) {
    if (existsSync(STOP_FILE)) {
      log("STOP présent — arrêt propre");
      break;
    }
    const result = await once(state);
    if (result.stopped) break;
    if (result.notExecuted) {
      log("aucun worker configuré : arrêt sans marquer de tâche terminée");
      break;
    }
    if (result.empty) {
      idle += 1;
      if (idle > 3) {
        log("plus rien à faire — arrêt (pas de boucle vide)");
        break;
      }
      await new Promise((r) => setTimeout(r, 60_000));
      continue;
    }
    idle = 0;
    if (result.failed || result.blocked) {
      log("échec : on ne contourne pas, on s'arrête et on laisse l'état");
      break;
    }
  }
  log("session terminée");
}

/* ────────────────────────────── MAIN ───────────────────────────────────── */

async function main() {
  const state = loadState();
  mkdirSync(STATE_DIR, { recursive: true });

  if (has("--status")) {
    const backlog = backlogStatus(state);
    console.log(
      JSON.stringify(
        {
          repo: REPO,
          lastRunAt: state.lastRunAt,
          current: state.current,
          backlog: {
            total: backlog.total,
            open: backlog.open,
            blocked: backlog.blocked,
            completed: backlog.completed,
          },
          next: backlog.next,
          currentStale: state.current !== null && !backlog.openTasks.includes(state.current) && !backlog.blockedTasks.some((t) => t.id === state.current),
          openTasks: backlog.openTasks,
          blockedTasks: backlog.blockedTasks,
          completedTasks: backlog.completedTasks,
          failedRuns: state.blocked ?? [],
          session: state.session,
        },
        null,
        2,
      ),
    );
    return;
  }

  if (has("--discover")) {
    const found = discover();
    writeJson(join(STATE_DIR, "discovery.json"), { at: new Date().toISOString(), ...found });
    console.log(`fichiers scannés : ${found.scanned}`);
    console.log(`TODO/FIXME : ${found.todos.length}`);
    for (const t of found.todos.slice(0, 40)) console.log(`  ${t.file}:${t.line}  ${t.text}`);
    console.log(`fichiers de test avec ignore/skip : ${found.ignoredTests.length}`);
    for (const f of found.ignoredTests) console.log(`  ${f}`);
    return;
  }

  if (has("--loop")) return loop(state);
  if (has("--once") || args.length === 0) return once(state);

  console.error("usage: --status | --discover | --once [--execute] [--commit] | --loop --hours N");
  process.exit(2);
}

main().catch((err) => {
  if (err.code === "BLOCKED") {
    log(`refus: ${err.message}`);
    console.error(`orchestrateur: ${err.message}`);
    process.exit(3);
    return;
  }
  log(`ERREUR orchestrateur: ${err.message}`);
  console.error(`orchestrateur: ${err.message}`);
  process.exit(1);
});
