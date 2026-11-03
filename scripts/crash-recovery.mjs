#!/usr/bin/env node
// crash-recovery.mjs — la preuve que F5 tient sur de vrais processus.
//
// F5 : « Un nœud qui meurt sans rendre son bail ne bloque aucune ressource
// au-delà de son bail. » Les tests unitaires le prouvent sur une horloge
// injectée ; ce script le prouve sur un daemon réel, tué par SIGKILL.
//
// Ce que le premier essai de ce script a appris, et qui a changé le scénario :
// tuer le CLIENT ne suffit pas à tester le bail. Le noyau ferme le socket, le
// serveur voit EOF et `LedgerGuard` libère — c'est correct, mais ça ne prouve
// rien sur la mort du processus. Le cas qui laisse vraiment une réservation
// orpheline est la mort du DAEMON (ou une coupure réseau), parce que le bail
// reste alors sur disque sans personne pour le rendre.
//
// Scénarios :
//   1. un client obtient un bail qui annonce une échéance réelle ;
//   2. ce client tué par SIGKILL est libéré par la fermeture du socket (le garde
//      fonctionne) — vérifié, pas supposé ;
//   3. un second bail est pris, puis le DAEMON est tué par SIGKILL : le bail
//      survit dans le ledger (durabilité) ;
//   4. après redémarrage et échéance, la même demande est acceptée et le bail
//      orphelin a disparu ;
//   5. le daemon redémarré répond encore.
//
// Zéro dépendance. Node >= 22.

import { spawn } from "node:child_process";
import { existsSync, readFileSync, mkdtempSync, rmSync } from "node:fs";
import { connect } from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";

const REPO = new URL("..", import.meta.url).pathname;
const DAEMON = join(REPO, "target", "debug", "polygoned");
const LEASE_SECS = 2;
const RAM_GB = 0.05;

const root = mkdtempSync(join(tmpdir(), "polygone-crash-"));
const socketPath = join(root, "daemon.sock");
const ledgerPath = join(root, "ledger.json");

const env = {
  ...process.env,
  POLYGONE_DAEMON_SOCKET: socketPath,
  POLYGONE_LEDGER_PATH: ledgerPath,
  POLYGONE_LEASE_SECS: String(LEASE_SECS),
  RUST_LOG: "error",
};

const failures = [];
const check = (ok, label, detail = "") => {
  console.log(`${ok ? "PASS" : "ÉCHEC"}  ${label}${detail ? ` — ${detail}` : ""}`);
  if (!ok) failures.push(label);
};

if (!existsSync(DAEMON)) {
  console.error(`binaire absent : ${DAEMON}\n→ cargo build -p polygoned`);
  process.exit(2);
}

let daemon = null;

function startDaemon() {
  daemon = spawn(DAEMON, [], { env, stdio: ["ignore", "ignore", "ignore"] });
  return daemon;
}

/** Attend une connexion RÉELLE, pas seulement le fichier : un socket orphelin
 *  existe après un SIGKILL et refuserait la connexion. */
async function connectWithRetry(timeoutMs = 10_000) {
  const deadline = Date.now() + timeoutMs;
  let lastError = null;
  while (Date.now() < deadline) {
    try {
      const conn = makeConnection();
      await conn.ready;
      const hello = await conn.read();
      if (hello.type !== "hello") throw new Error(`attendu hello, reçu ${hello.type}`);
      return conn;
    } catch (err) {
      lastError = err;
      await sleep(100);
    }
  }
  throw new Error(`connexion impossible : ${lastError?.message ?? "inconnu"}`);
}

function makeConnection() {
  const socket = connect(socketPath);
  let buffer = "";
  const queue = [];
  const waiters = [];
  socket.setEncoding("utf8");
  socket.on("data", (chunk) => {
    buffer += chunk;
    let index;
    while ((index = buffer.indexOf("\n")) >= 0) {
      const line = buffer.slice(0, index).trim();
      buffer = buffer.slice(index + 1);
      if (!line) continue;
      const parsed = JSON.parse(line);
      const waiter = waiters.shift();
      if (waiter) waiter(parsed);
      else queue.push(parsed);
    }
  });
  const read = () =>
    new Promise((resolve, reject) => {
      const ready = queue.shift();
      if (ready) return resolve(ready);
      const timer = setTimeout(() => reject(new Error("aucune réponse du daemon")), 5_000);
      waiters.push((value) => {
        clearTimeout(timer);
        resolve(value);
      });
    });
  const ready = new Promise((resolve, reject) => {
    socket.once("connect", resolve);
    socket.once("error", reject);
  });
  ready.catch(() => {});
  const alloc = async (client) => {
    socket.write(
      JSON.stringify({
        v: 1,
        type: "alloc_request",
        client,
        cpu_cores: 1,
        ram_gb: RAM_GB,
      }) + "\n",
    );
    return read();
  };
  return { socket, read, ready, alloc };
}

function readLedger() {
  if (!existsSync(ledgerPath)) return { grants: {} };
  return JSON.parse(readFileSync(ledgerPath, "utf8"));
}

const grantIds = () => Object.keys(readLedger().grants ?? {});

/* ─────────────────────────── scénario ─────────────────────────── */

startDaemon();
let conn = await connectWithRetry();

// 1. Un bail avec une échéance réelle.
const first = await conn.alloc("premier");
check(first.type === "alloc_granted", "1. un bail est accordé", first.type);
check(
  typeof first.expires_at === "number",
  "1b. le bail annonce une échéance réelle",
  `expires_at=${first.expires_at}`,
);
const firstId = first.allocation_id;
check(grantIds().includes(firstId), "1c. le bail est persisté sur disque", grantIds().join(", "));

// 2. Client tué par SIGKILL : le garde libère via la fermeture du socket.
const consumer = spawn(
  process.execPath,
  [
    "-e",
    `
    const { connect } = require("node:net");
    const s = connect(${JSON.stringify(socketPath)});
    s.on("ready", () => s.write(JSON.stringify({ v: 1, type: "alloc_request",
      client: "tue", cpu_cores: 1, ram_gb: ${RAM_GB} }) + "\\n"));
    setInterval(() => {}, 1000);
    `,
  ],
  { stdio: "ignore" },
);
await sleep(1200);
const duringConsumer = grantIds().length;
consumer.kill("SIGKILL");
await sleep(1200);
const afterConsumer = grantIds().length;
check(
  afterConsumer < duringConsumer,
  "2. le bail du client tué est rendu (le garde fonctionne)",
  `${duringConsumer} → ${afterConsumer} bail(aux)`,
);

// 3. Daemon tué pendant qu'un bail est tenu : le bail survit sur disque.
const held = await conn.alloc("tenu-pendant-crash");
check(held.type === "alloc_granted", "3. un bail est accordé avant le crash du daemon", held.type);
const heldId = held.allocation_id;
daemon.kill("SIGKILL");
await sleep(400);
check(
  grantIds().includes(heldId),
  "3b. le bail survit au SIGKILL du daemon (durabilité du ledger)",
  grantIds().join(", "),
);

// 4. Redémarrage, échéance passée : la marge doit revenir.
startDaemon();
conn = await connectWithRetry();
await sleep(LEASE_SECS * 1000 + 800);

const afterExpiry = await conn.alloc("apres-expiration");
check(
  afterExpiry.type === "alloc_granted",
  "4. après l'échéance, la même demande est acceptée",
  `${afterExpiry.type} ${afterExpiry.reason ?? ""}`.trim(),
);
check(
  !grantIds().includes(heldId),
  "4b. le bail orphelin a disparu du ledger",
  `reste : ${grantIds().join(", ") || "aucun"}`,
);

// 5. Le daemon redémarré répond encore.
conn.socket.write(JSON.stringify({ v: 1, type: "ping", id: "final" }) + "\n");
const pong = await conn.read();
check(pong.type === "pong", "5. le daemon redémarré répond", pong.type);
conn.socket.end();

daemon.kill("SIGKILL");
await sleep(200);
rmSync(root, { recursive: true, force: true });

console.log("");
if (failures.length) {
  console.error(`${failures.length} vérification(s) en échec : ${failures.join(" ; ")}`);
  process.exit(1);
}
console.log("crash-recovery : tous les scénarios passent");
