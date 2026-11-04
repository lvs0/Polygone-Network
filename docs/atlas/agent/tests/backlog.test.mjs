// Tests de l'orchestrateur contre son backlog (AGT.1, AGT.2).
//
// Ces tests lancent le VRAI script, jamais une copie de sa logique : une
// réimplémentation dans le test passerait au vert pendant que la boucle casse.
// L'état est isolé dans un dépôt temporaire via POLYGONE_DISPATCH.

import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const AGENT_DIR = dirname(dirname(fileURLToPath(import.meta.url)));
const ORCHESTRATOR = join(AGENT_DIR, "orchestrateur.mjs");
const BACKLOG = join(AGENT_DIR, "backlog.json");

/** Dépôt jetable : l'orchestrateur y écrit son état, pas dans le vrai. */
function fakeRepo(completed = [], blocked = []) {
  const root = mkdtempSync(join(tmpdir(), "polygone-agent-"));
  mkdirSync(join(root, ".orchestrator"), { recursive: true });
  writeFileSync(
    join(root, ".orchestrator", "state.json"),
    JSON.stringify({ completed, blocked, session: { tasks: 0, commits: 0, failedChecks: 0 } }),
  );
  const config = join(root, "dispatch.json");
  writeFileSync(config, JSON.stringify({ repo: root, agentCommand: null, checks: [] }));
  return { root, config };
}

function run(config, ...args) {
  try {
    const stdout = execFileSync("node", [ORCHESTRATOR, ...args], {
      env: { ...process.env, POLYGONE_DISPATCH: config },
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    });
    return { code: 0, stdout, stderr: "" };
  } catch (err) {
    return { code: err.status ?? 1, stdout: err.stdout ?? "", stderr: err.stderr ?? "" };
  }
}

const backlog = JSON.parse(readFileSync(BACKLOG, "utf8"));
const blockedIds = backlog.tasks.filter((t) => t.blocked_by).map((t) => t.id);

test("le backlog est lisible et cohérent", () => {
  assert.ok(backlog.tasks.length > 0, "le backlog ne peut pas être vide");
  const ids = new Set();
  for (const task of backlog.tasks) {
    assert.equal(typeof task.id, "string", "chaque tâche a un identifiant");
    assert.ok(!ids.has(task.id), `identifiant en double : ${task.id}`);
    ids.add(task.id);
    assert.ok(task.phase, `${task.id} n'a pas de phase`);
    assert.ok(Number.isFinite(Number(task.priority)), `${task.id} n'a pas de priorité numérique`);
  }
  // La forme structurée est ce que lit l'orchestrateur ; une chaîne en prose ne
  // bloque pas, elle est signalée comme non structurée.
  for (const task of backlog.tasks.filter((t) => t.blocked_by)) {
    assert.equal(typeof task.blocked_by, "object", `${task.id} : blocked_by doit être structuré`);
  }
});

test("--status ne compte jamais une tâche bloquée comme ouverte", () => {
  const { config } = fakeRepo();
  const { code, stdout } = run(config, "--status");
  assert.equal(code, 0, "status doit réussir");
  const status = JSON.parse(stdout);

  assert.equal(status.backlog.total, backlog.tasks.length + backlog.completed.length);
  assert.equal(status.backlog.completed, backlog.completed.length);
  for (const item of backlog.completed) {
    assert.ok(status.completedTasks.includes(item.id), item.id + " doit rester terminé");
    assert.ok(!status.openTasks.includes(item.id), item.id + " terminé ne doit pas redevenir ouvert");
  }
  for (const id of blockedIds) {
    assert.ok(!status.openTasks.includes(id), `${id} bloquée ne doit pas être « ouverte »`);
    const entry = status.blockedTasks.find((t) => t.id === id);
    assert.ok(entry, `${id} doit apparaître dans blockedTasks`);
    assert.ok(entry.blockers.length > 0, `${id} doit nommer au moins un bloqueur`);
  }
  assert.equal(
    status.backlog.open + status.backlog.blocked + status.backlog.completed,
    status.backlog.total,
    "les trois catégories partitionnent le backlog",
  );
});

test("une tâche bloquée est refusée en nommant son bloqueur", () => {
  const { config } = fakeRepo();
  const id = blockedIds[0];
  assert.ok(id, "le backlog doit contenir au moins une tâche bloquée");
  const { code, stderr } = run(config, "--once", "--task", id);
  assert.equal(code, 3, "une tâche bloquée sort avec le code 3, pas 0");
  assert.match(stderr, new RegExp(id));
  assert.match(stderr, /BLOQUÉE/);
});

test("débloquer une tâche demande que sa dépendance soit terminée", () => {
  const withDeps = backlog.tasks.filter((t) => t.blocked_by?.tasks?.length);
  assert.ok(withDeps.length > 0, "au moins une tâche dépend d'une autre tâche");
  const task = withDeps[0];
  const dep = task.blocked_by.tasks[0];

  // Sans la dépendance : refus, et c'est la dépendance qui est nommée.
  const before = fakeRepo();
  const refused = run(before.config, "--once", "--task", task.id);
  assert.equal(refused.code, 3);
  assert.match(refused.stderr, new RegExp(dep));

  // Avec la dépendance terminée mais une décision humaine encore ouverte : le
  // refus doit nommer la décision, pas la tâche déjà faite.
  if (task.blocked_by.decisions?.length) {
    const after = fakeRepo([dep]);
    const stillBlocked = run(after.config, "--once", "--task", task.id);
    assert.equal(stillBlocked.code, 3);
    assert.match(stillBlocked.stderr, new RegExp(task.blocked_by.decisions[0]));
  }
});

test("la tâche proposée est la première du backlog, pas la première du fichier", () => {
  const { config } = fakeRepo();
  const { code, stdout } = run(config, "--status");
  assert.equal(code, 0);
  const status = JSON.parse(stdout);

  const completedIds = new Set(backlog.completed.map((item) => item.id));
  const open = backlog.tasks
    .filter((t) => !completedIds.has(t.id) && !t.blocked_by && t.id !== status.blockedTasks.find((b) => b.id === t.id)?.id)
    .sort((a, b) => a.phase.localeCompare(b.phase) || Number(a.priority) - Number(b.priority));

  if (status.next) {
    assert.equal(status.next, open[0].id, "l'ordre est (phase, priorité), pas l'ordre du fichier");
  }
});


test("un worker absent ne termine jamais une tâche", () => {
  const { root, config } = fakeRepo();
  const { code, stdout } = run(config, "--once", "--execute");
  assert.equal(code, 0, "l'absence de worker doit être un arrêt sûr");
  assert.match(stdout, /aucun agentCommand configuré/);
  const state = JSON.parse(readFileSync(join(root, ".orchestrator", "state.json"), "utf8"));
  assert.deepEqual(state.completed, [], "aucune tâche ne doit être cochée");
  assert.equal(state.current, null, "aucune tâche non exécutée ne doit rester en cours");
});
