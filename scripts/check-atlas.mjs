#!/usr/bin/env node
import { readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
const root = dirname(dirname(fileURLToPath(import.meta.url)));
const atlas = join(root, "docs", "atlas");
const fail = [];
const required = ["README.md","STATUS-2026-11-04.md","audit/INVENTAIRE.md","plan/PLAN-MAITRE.md","plan/PLAN-FINITION.md","plan/PLAN-ECOSYSTEME.md","plan/DECISION-POLY.md","agent/MISSION-FINALISATION-PRODUIT.md","agent/ORDRE.md","agent/backlog.json","agent/dispatch.json","agent/orchestrateur.mjs","agent/tests/backlog.test.mjs","ecosystem/HOPE-GATEWAY.md"];
for (const f of required) { try { statSync(join(atlas,f)); } catch { fail.push("fichier manquant : "+f); } }
const epicDir = join(atlas,"plan","epiques");
const epics = readdirSync(epicDir).filter((f)=>f.endsWith(".md"));
const ids = [];
for (const f of epics) for (const m of readFileSync(join(epicDir,f),"utf8").matchAll(/^###\s+([A-Z]+\.\d+)\b/gm)) ids.push(m[1]);
if (epics.length !== 6) fail.push("6 épiques attendus, "+epics.length+" trouvés");
if (ids.length !== 132 || new Set(ids).size !== 132) fail.push("132 identifiants uniques attendus, "+ids.length+" trouvés");
const backlog = JSON.parse(readFileSync(join(atlas,"agent","backlog.json"),"utf8"));
if (!Array.isArray(backlog.tasks) || !Array.isArray(backlog.completed)) fail.push("backlog.tasks/completed invalides");
const backlogIds = [...backlog.tasks.map(x=>x.id),...backlog.completed.map(x=>x.id)];
if (new Set(backlogIds).size !== backlogIds.length) fail.push("identifiants backlog dupliqués");
for (const t of backlog.tasks) if (!t.id || !t.phase || !Number.isFinite(Number(t.priority))) fail.push("tâche backlog invalide : "+(t.id||"sans id"));
const files=[];
function walk(dir){for(const e of readdirSync(dir,{withFileTypes:true})){const p=join(dir,e.name);if(e.isDirectory())walk(p);else if(/\.(md|json|mjs)$/.test(e.name))files.push(p);}}
walk(atlas);
const forbidden=[/\/home\/(?:lvs0|l-vs)\//,/~\/(?:Polygone-Atlas|freebuff|hope-gateway)/,/user_corpus\.jsonl/,/vision-long\.md/];
for(const f of files){const s=readFileSync(f,"utf8");for(const r of forbidden)if(r.test(s))fail.push("chemin privé dans "+relative(root,f));}
if(fail.length){console.error("Atlas check: FAIL");for(const x of fail)console.error("- "+x);process.exit(1);}
const epicSet=new Set(ids);const overlap=backlogIds.filter(x=>epicSet.has(x)).length;
console.log("Atlas check: PASS — "+epics.length+" épiques, "+ids.length+" tâches spécifiées, "+backlog.tasks.length+" ouvertes, "+backlog.completed.length+" terminées, "+(ids.length-overlap)+" non promues ; aucun chemin privé détecté.");
