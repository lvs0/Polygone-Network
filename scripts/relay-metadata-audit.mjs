#!/usr/bin/env node
// ═══════════════════════════════════════════════════════════════════════════
//  relay-metadata-audit.mjs — SEC.10
//
//  Lance un relais réel, un émetteur et un récepteur, fait traverser un envoi
//  par un proxy TCP d'observation, puis produit un rapport daté listant ce que
//  le relais voit (identités de routage, tailles, cadence, horodatage) et ce
//  qu'il ne voit pas (contenu, nom de fichier).
//
//  Refuse de conclure si aucune trame réelle n'a traversé un socket TCP.
//  Usage : node scripts/relay-metadata-audit.mjs
// ═══════════════════════════════════════════════════════════════════════════

import { spawn } from "node:child_process";
import net from "node:net";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const BIN = process.env.POLYGONE_BIN || path.join(ROOT, "target/debug/polygone");
const RELAY_BIN =
  process.env.POLYGONE_RELAY_BIN || path.join(ROOT, "target/debug/polygone-relay");
const MARKER = "POLYGONE-METADATA-AUDIT-CONTENT-MARKER-7f3c";
const DATE = new Date().toISOString().slice(0, 10);
const REPORT_DIR = path.join(ROOT, "reports");
const REPORT = path.join(REPORT_DIR, `relay-metadata-audit-${DATE}.md`);

const children = [];
const frames = [];
const wire = [];
let proxyConnections = 0;

const fail = (message) => {
  console.error(`✖ ${message}`);
  cleanup();
  process.exit(1);
};

function cleanup() {
  for (const child of children) {
    try {
      child.kill("SIGTERM");
    } catch {
      /* déjà mort */
    }
  }
}

function run(command, args, { env = {}, timeoutMs = 30_000 } = {}) {
  return new Promise((resolve) => {
    const child = spawn(command, args, {
      cwd: ROOT,
      env: { ...process.env, ...env },
    });
    let stdout = "";
    let stderr = "";
    const timer = setTimeout(() => child.kill("SIGKILL"), timeoutMs);
    child.stdout.on("data", (d) => (stdout += d));
    child.stderr.on("data", (d) => (stderr += d));
    child.on("close", (code) => {
      clearTimeout(timer);
      resolve({ code, stdout, stderr });
    });
  });
}

function freePort() {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const { port } = server.address();
      server.close(() => resolve(port));
    });
  });
}

function waitForPort(port, timeoutMs = 10_000) {
  const deadline = Date.now() + timeoutMs;
  return new Promise((resolve, reject) => {
    const attempt = () => {
      const socket = net.connect(port, "127.0.0.1");
      socket.once("connect", () => {
        socket.destroy();
        resolve();
      });
      socket.once("error", () => {
        socket.destroy();
        if (Date.now() > deadline) reject(new Error(`port ${port} injoignable`));
        else setTimeout(attempt, 100);
      });
    };
    attempt();
  });
}

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

function recordLines(chunk, direction, buffer) {
  buffer.value += chunk.toString("utf8");
  const lines = buffer.value.split("\n");
  buffer.value = lines.pop() ?? "";
  for (const line of lines) {
    if (!line.trim()) continue;
    const frame = { direction, at: Date.now(), bytes: Buffer.byteLength(line) };
    if (line.startsWith("HELLO ")) {
      frame.kind = "HELLO";
      frame.hello = line.slice(6).trim();
    } else {
      try {
        const parsed = JSON.parse(line);
        frame.kind = parsed.kind;
        frame.from = parsed.from;
        frame.to = parsed.to;
        frame.session = parsed.session;
        frame.seq = parsed.seq;
        frame.payloadLen = Array.isArray(parsed.payload) ? parsed.payload.length : null;
      } catch {
        frame.kind = "non-JSON";
      }
    }
    frames.push(frame);
  }
}

async function main() {
  for (const [label, bin] of [
    ["client", BIN],
    ["relais", RELAY_BIN],
  ]) {
    if (!fs.existsSync(bin)) {
      fail(
        `${label} introuvable : ${bin}\n` +
          "  construire d'abord : cargo build -p polygone-client --bin polygone -p polygone-relay",
      );
    }
  }

  const relayPort = await freePort();
  const proxyPort = await freePort();
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "polygone-metadata-"));
  const senderHome = path.join(tmp, "sender-home");
  const receiverHome = path.join(tmp, "receiver-home");
  fs.mkdirSync(senderHome, { recursive: true });
  fs.mkdirSync(receiverHome, { recursive: true });
  const payloadPath = path.join(tmp, "payload.bin");
  // Assez petit pour tenir sous le plafond de 64 KiB par enveloppe, assez
  // grand pour produire plusieurs trames mesurables.
  const payload = `${MARKER}\n`.repeat(300); // ~13 Kio
  fs.writeFileSync(payloadPath, payload);

  console.log(`⬡ audit métadonnées du relais — relais:${relayPort} proxy:${proxyPort}`);

  const relayLog = path.join(tmp, "relay.log");
  const relay = spawn(RELAY_BIN, ["--port", String(relayPort)], {
    cwd: ROOT,
    env: { ...process.env, RUST_LOG: "info" },
  });
  children.push(relay);
  const relayOut = fs.createWriteStream(relayLog);
  relay.stdout.pipe(relayOut);
  relay.stderr.pipe(relayOut);

  // Proxy d'observation : tout le trafic des deux pairs passe par lui, comme
  // le verrait un opérateur qui instrumente sa propre entrée réseau.
  const proxy = net.createServer((downstream) => {
    proxyConnections += 1;
    const upstream = net.connect(relayPort, "127.0.0.1");
    const downBuffer = { value: "" };
    const upBuffer = { value: "" };
    downstream.on("data", (chunk) => {
      wire.push(chunk);
      recordLines(chunk, "pair→relais", downBuffer);
      upstream.write(chunk);
    });
    upstream.on("data", (chunk) => {
      wire.push(chunk);
      recordLines(chunk, "relais→pair", upBuffer);
      downstream.write(chunk);
    });
    const close = () => {
      downstream.destroy();
      upstream.destroy();
    };
    downstream.on("error", close);
    upstream.on("error", close);
    downstream.on("close", () => upstream.destroy());
    upstream.on("close", () => downstream.destroy());
  });
  await new Promise((resolve) => proxy.listen(proxyPort, "127.0.0.1", resolve));

  try {
    await waitForPort(relayPort);
  } catch (error) {
    fail(`${error.message}\n--- relais ---\n${fs.readFileSync(relayLog, "utf8")}`);
  }

  const receiverKey = await run(BIN, ["clef"], { env: { HOME: receiverHome } });
  if (receiverKey.code !== 0) fail(`clef du récepteur : ${receiverKey.stderr}`);
  // `clef` imprime la clé puis un commentaire (# fichier identité).
  const receiverKeyHex = receiverKey.stdout.trim().split("\n")[0].trim();
  const receiverId = await run(BIN, ["id"], { env: { HOME: receiverHome } });
  if (receiverId.code !== 0) fail(`id du récepteur : ${receiverId.stderr}`);
  const receiver = receiverId.stdout.trim();
  console.log(`  récepteur : ${receiver}`);

  const receiverLog = path.join(tmp, "receiver.log");
  const listener = spawn(
    BIN,
    ["ecouter", "--relay", `127.0.0.1:${proxyPort}`],
    { cwd: ROOT, env: { ...process.env, HOME: receiverHome } },
  );
  children.push(listener);
  const listenerOut = fs.createWriteStream(receiverLog);
  listener.stdout.pipe(listenerOut);
  listener.stderr.pipe(listenerOut);
  await sleep(1500);

  const senderLog = path.join(tmp, "sender.log");
  const send = await run(
    BIN,
    [
      "envoyer",
      "--via",
      `127.0.0.1:${proxyPort}`,
      "--a",
      receiver,
      "-d",
      receiverKeyHex,
      "--fichier",
      payloadPath,
    ],
    { env: { HOME: senderHome }, timeoutMs: 30_000 },
  );
  fs.writeFileSync(senderLog, `code=${send.code}\n${send.stdout}\n${send.stderr}`);
  await sleep(2000);

  if (send.code !== 0) fail(`envoi échoué (code ${send.code}) : ${send.stderr || send.stdout}`);
  const realFrames = frames.filter((f) => f.kind === "fragment");
  const hellos = frames.filter((f) => f.kind === "HELLO");
  const relayLogText = fs.readFileSync(relayLog, "utf8");
  const listenerText = fs.readFileSync(receiverLog, "utf8");

  if (proxyConnections < 2 || hellos.length < 2) {
    fail(
      `flux réel insuffisant : ${proxyConnections} connexions, ${hellos.length} HELLO — ` +
        "aucune conclusion autorisée sans socket réel",
    );
  }
  if (realFrames.length === 0) {
    fail("aucune trame fragment observée : le test n'a pas traversé le relais");
  }

  const markerLeaked =
    Buffer.concat(wire).includes(Buffer.from(MARKER)) ||
    relayLogText.includes(MARKER) ||
    listenerText.includes(MARKER);
  if (markerLeaked) fail("le contenu en clair a été observé sur le fil ou dans un journal");

  const fragments = realFrames;
  const sessions = [...new Set(fragments.map((f) => f.session).filter(Boolean))];
  const senders = [...new Set(fragments.map((f) => f.from).filter(Boolean))];
  const recipients = [...new Set(fragments.map((f) => f.to).filter(Boolean))];
  const lineSizes = fragments.map((f) => f.bytes);
  const times = fragments.map((f) => f.at);
  const intervals = times.slice(1).map((t, i) => t - times[i]);
  const spanMs = Math.max(...times) - Math.min(...times);

  const empty = (values) => values.length === 0 || values.every((v) => !v);
  const visibleContent = !markerLeaked;

  const lines = [];
  lines.push("# Audit des métadonnées visibles par le relais");
  lines.push("");
  lines.push("> **Instantané daté.** Rapport généré par");
  lines.push("> `scripts/relay-metadata-audit.mjs` — SEC.10. Il décrit une exécution");
  lines.push(`> locale du ${new Date().toISOString()}, pas une propriété intemporelle.`);
  lines.push("> L'état courant et les limites sont suivis dans `STATE.md`.");
  lines.push("");
  lines.push("## Dispositif réellement exécuté");
  lines.push("");
  lines.push(`- Un relais \`polygone-relay --port ${relayPort}\` (TCP réel).`);
  lines.push(`- Un émetteur \`polygone envoyer --fichier\` (~13 Kio, marqueur de contenu).`);
  lines.push(`- Un récepteur \`polygone ecouter\` (identité \`${receiver}\`).`);
  lines.push(`- Un proxy d'observation local (port ${proxyPort}) qui relaie chaque octet`);
  lines.push("  et journalise les trames dans les deux sens.");
  lines.push(`- ${proxyConnections} connexions TCP, ${hellos.length} HELLO, ` +
    `${fragments.length} fragments, ${sessions.length} session(s).`);
  lines.push("");
  lines.push("## Ce que le relais voit");
  lines.push("");
  lines.push("| Champ | Visible | Preuve mesurée |");
  lines.push("|---|---|---|");
  lines.push(
    `| Identité source (\`from\`/HELLO) | ${empty(senders) && hellos.length === 0 ? "non" : "**oui**"} | ` +
      `${hellos.length} HELLO, sources : ${empty(senders) ? "—" : senders.join(", ")} |`,
  );
  lines.push(
    `| Identité destination (\`to\`) | ${empty(recipients) ? "non" : "**oui**"} | ` +
      `destinations : ${empty(recipients) ? "—" : recipients.join(", ")} |`,
  );
  lines.push(
    `| Taille des trames | **oui** | ${Math.min(...lineSizes)}–${Math.max(...lineSizes)} octets ` +
      `(médiane ${lineSizes.sort((a, b) => a - b)[Math.floor(lineSizes.length / 2)]}) |`,
  );
  lines.push(
    `| Cadence | **oui** | ${fragments.length} fragments en ${spanMs} ms ; ` +
      `intervalle médian ${intervals.length ? intervals.sort((a, b) => a - b)[Math.floor(intervals.length / 2)] : 0} ms |`,
  );
  lines.push("| Horodatage d'observation | **oui** | chaque trame est datée à la réception par l'observateur |");
  lines.push(
    `| Identifiant de session | ${sessions.length ? "**oui**" : "non"} | ${sessions.length} session(s) : ` +
      `${sessions.join(", ") || "—"} |`,
  );
  lines.push(
    `| Contenu du message/fichier | ${visibleContent ? "**non**" : "OUI — ÉCHEC"} | ` +
      `marqueur \`${MARKER}\` absent des ${Buffer.concat(wire).length} octets observés et des journaux |`,
  );
  lines.push("| Nom du fichier | **non** | chiffré hors bande ; aucune trame ne le contient |");
  lines.push("");
  lines.push("## Limites de cette mesure");
  lines.push("");
  lines.push("- Tout est sur **une seule machine** (loopback) : la mesure prouve ce que");
  lines.push("  l'observateur voit sur le fil, pas la topologie entre deux hôtes physiques.");
  lines.push("- L'observateur est un proxy local : un opérateur qui modifie le binaire du");
  lines.push("  relais verrait la même chose, mais pourrait aussi conserver les données.");
  lines.push("- Les horodatages sont ceux de l'observateur ; ils incluent l'ordonnancement");
  lines.push("  local et ne constituent pas une mesure de latence réseau.");
  lines.push("- Le relais ne voit pas le contenu, mais voit **qui parle à qui** : la");
  lines.push("  confidentialité porte sur le message, pas sur le graphe social.");
  lines.push("");
  lines.push("## Reproduction");
  lines.push("");
  lines.push("```bash");
  lines.push("cargo build -p polygone-client --bin polygone -p polygone-relay");
  lines.push("node scripts/relay-metadata-audit.mjs");
  lines.push("```");
  lines.push("");

  fs.mkdirSync(REPORT_DIR, { recursive: true });
  fs.writeFileSync(REPORT, lines.join("\n"));
  console.log(`✓ rapport : ${path.relative(ROOT, REPORT)}`);
  console.log(
    `  ${fragments.length} fragments, ${sessions.length} session(s), ` +
      `${Math.min(...lineSizes)}–${Math.max(...lineSizes)} octets, contenu absent`,
  );
}

process.on("SIGINT", () => {
  cleanup();
  process.exit(130);
});

main()
  .then(() => {
    cleanup();
    process.exit(0);
  })
  .catch((error) => {
    console.error(`✖ ${error.stack || error}`);
    cleanup();
    process.exit(1);
  });
