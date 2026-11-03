/* ──────────────────────────────────────────────────────────────────
   Polygone — node dashboard logic
   Fetches /api/status every 3s and renders live stats.

   When the daemon does not answer, every value is shown as unknown ("—").
   Nothing here invents a number: a dashboard that lies is worse than a
   dashboard that says "je ne sais pas".
   ────────────────────────────────────────────────────────────────── */

(() => {
  "use strict";

  const ENDPOINT = "/api/status";
  const POLL_MS = 3000;

  // ── State ────────────────────────────────────────────────────────
  // Tout est inconnu tant que le daemon n'a pas répondu. Aucune valeur
  // par défaut n'est plausible : une valeur plausible est un mensonge
  // qu'on ne remarque pas.
  const UNKNOWN = null;
  const state = {
    available: false,
    lastOkAt: null,
    uptime: UNKNOWN,
    peers: UNKNOWN,
    traffic_in: UNKNOWN,
    traffic_out: UNKNOWN,
    frag_ready: UNKNOWN,
    frag_needed: UNKNOWN,
    modules: [],
    log: [],
    spark_in: [],
    spark_out: [],
  };

  const resetUnknown = () => {
    state.available = false;
    state.uptime = UNKNOWN;
    state.peers = UNKNOWN;
    state.traffic_in = UNKNOWN;
    state.traffic_out = UNKNOWN;
    state.frag_ready = UNKNOWN;
    state.frag_needed = UNKNOWN;
    state.modules = [];
    state.log = [];
    state.spark_in = [];
    state.spark_out = [];
  };

  // ── DOM refs ─────────────────────────────────────────────────────
  const $ = (id) => document.getElementById(id);
  const elUptime    = $("uptime");
  const elPeers     = $("peers");
  const elTrafficIn = $("traffic-in");
  const elTrafficOut= $("traffic-out");
  const elFragReady = $("frag-ready");
  const elFragNeed  = $("frag-needed");
  const elFragBar   = $("frag-bar");
  const elModules   = $("modules");
  const elLog       = $("log");
  const sparkIn     = $("spark-in").querySelector("polyline");
  const sparkOut    = $("spark-out").querySelector("polyline");

  const moduleTpl = document.getElementById("module-template");
  const logTpl    = document.getElementById("log-template");

  // ── Format helpers ───────────────────────────────────────────────
  const fmtUptime = (s) => {
    const h = String(Math.floor(s / 3600)).padStart(2, "0");
    const m = String(Math.floor((s % 3600) / 60)).padStart(2, "0");
    const sec = String(s % 60).padStart(2, "0");
    return `${h}:${m}:${sec}`;
  };
  const fmtBytes = (n) => {
    if (n < 1024) return `${n.toFixed(0)} b/s`;
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB/s`;
    return `${(n / 1024 / 1024).toFixed(2)} MB/s`;
  };
  const DASH = "—";
  const shown = (value, format) => (Number.isFinite(value) ? format(value) : DASH);

  // ── Render functions ─────────────────────────────────────────────
  const renderBanner = () => {
    elUptime.textContent = shown(state.uptime, fmtUptime);
    elPeers.textContent  = shown(state.peers, (n) => String(n));
    document.body.dataset.node = state.available ? "online" : "unknown";
  };

  const renderStats = () => {
    const setRate = (el, value) => {
      el.firstChild.textContent = shown(value, (n) => fmtBytes(n).replace(/ b\/s$/, ""));
      el.querySelector("span").textContent = Number.isFinite(value) ? "b/s" : "";
    };
    setRate(elTrafficIn, state.traffic_in);
    setRate(elTrafficOut, state.traffic_out);
    elFragReady.textContent = shown(state.frag_ready, (n) => String(n));
    elFragNeed.textContent  = shown(state.frag_needed, (n) => String(n));
    const ratio = Number.isFinite(state.frag_ready) && Number.isFinite(state.frag_needed) && state.frag_needed > 0
      ? Math.min(100, (state.frag_ready / state.frag_needed) * 100)
      : 0;
    elFragBar.style.width = `${ratio}%`;
  };

  const renderSpark = () => {
    const toPath = (arr) => arr.map((v, i) => {
      const x = (i / Math.max(1, arr.length - 1)) * 100;
      const max = Math.max(...arr, 1);
      const y = 24 - (v / max) * 22 - 1;
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    }).join(" ");
    sparkIn.setAttribute("points",  toPath(state.spark_in));
    sparkOut.setAttribute("points", toPath(state.spark_out));
  };

  const renderModules = () => {
    elModules.innerHTML = "";
    if (!state.modules.length) {
      elModules.dataset.empty = "true";
      elModules.textContent = state.available ? "aucun module annoncé" : DASH;
      return;
    }
    delete elModules.dataset.empty;
    state.modules.forEach((m) => {
      const node = moduleTpl.content.firstElementChild.cloneNode(true);
      node.dataset.status = m.status;
      node.querySelector(".module__icon").textContent   = m.icon;
      node.querySelector(".module__name").textContent   = m.name;
      node.querySelector(".module__status").textContent = m.label;
      elModules.appendChild(node);
    });
  };

  const renderLog = () => {
    elLog.innerHTML = "";
    state.log.slice().reverse().forEach((entry) => {
      const node = logTpl.content.firstElementChild.cloneNode(true);
      node.dataset.kind = entry.kind;
      const m = Math.floor((entry.t % 3600) / 60);
      const s = entry.t % 60;
      node.querySelector(".log__time").textContent =
        `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
      node.querySelector(".log__msg").textContent = entry.msg;
      elLog.appendChild(node);
    });
  };

  const render = () => {
    renderBanner();
    renderStats();
    renderSpark();
    renderModules();
    renderLog();
  };

  // ── Network fetch ────────────────────────────────────────────────
  // Le sondage ne s'arrête JAMAIS : un tableau de bord figé sur un succès
  // ancien finit par mentir, et personne ne le remarque.
  const fetchStatus = async () => {
    try {
      const res = await fetch(ENDPOINT, { cache: "no-store" });
      if (!res.ok) throw new Error(String(res.status));
      const data = await res.json();
      Object.assign(state, {
        uptime: data.uptime ?? UNKNOWN,
        peers: data.peers ?? UNKNOWN,
        traffic_in: data.traffic_in ?? UNKNOWN,
        traffic_out: data.traffic_out ?? UNKNOWN,
        frag_ready: data.frag_ready ?? UNKNOWN,
        frag_needed: data.frag_needed ?? UNKNOWN,
        modules: Array.isArray(data.modules) ? data.modules : [],
        log: Array.isArray(data.log) ? data.log.slice(-30) : [],
      });
      if (Number.isFinite(state.traffic_in)) {
        state.spark_in.push(state.traffic_in);
        if (state.spark_in.length > 40) state.spark_in.shift();
      }
      if (Number.isFinite(state.traffic_out)) {
        state.spark_out.push(state.traffic_out);
        if (state.spark_out.length > 40) state.spark_out.shift();
      }
      state.available = true;
      state.lastOkAt = Date.now();
    } catch {
      resetUnknown();
    }
    render();
  };

  // ── Init ─────────────────────────────────────────────────────────
  resetUnknown();
  render();
  setInterval(() => { render(); }, 1000);
  setInterval(fetchStatus, POLL_MS);
  fetchStatus();
})();
