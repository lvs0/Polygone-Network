/* ──────────────────────────────────────────────────────────────────
   Polygone — node dashboard logic
   Fetches /api/status every 3s, renders live stats.
   Falls back to a deterministic local simulation when offline.
   ────────────────────────────────────────────────────────────────── */

(() => {
  "use strict";

  const ENDPOINT = "/api/status";
  const POLL_MS = 3000;

  // ── State ────────────────────────────────────────────────────────
  const state = {
    uptime: 0,
    peers: 0,
    traffic_in: 0,
    traffic_out: 0,
    frag_ready: 3,
    frag_needed: 4,
    balance: 10,
    consumption: 0.1,
    modules: [
      { name: "Msg",   icon: "💬", status: "running", label: "Running" },
      { name: "Hide",  icon: "👻", status: "running", label: "Running" },
      { name: "Drive", icon: "📁", status: "off",     label: "Off" },
      { name: "Mesh",  icon: "🔗", status: "off",     label: "Off" },
      { name: "Brain", icon: "🧠", status: "soon",    label: "Soon" },
    ],
    log: [
      { t: 1080, kind: "success", msg: "⬡ Polygone v1.0.0 démarré" },
      { t: 1020, kind: "info",    msg: "Clé ML-KEM-1024 générée" },
      { t: 900,  kind: "success", msg: "Pairing 127.0.0.1:4001 ✓" },
      { t: 720,  kind: "warn",    msg: "Cache zéroé — 4.2 MB libérés" },
      { t: 480,  kind: "success", msg: "Test Shamir 4-of-7 : 35/35 ✓" },
      { t: 300,  kind: "info",    msg: "Tunnel Hide SOCKS5 ready :9050" },
    ],
    spark_in: [],
    spark_out: [],
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
  const elBalance   = $("balance");
  const elConsump   = $("consumption");
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

  // ── Render functions ─────────────────────────────────────────────
  const renderBanner = () => {
    elUptime.textContent = fmtUptime(state.uptime);
    elPeers.textContent  = state.peers;
  };

  const renderStats = () => {
    elTrafficIn.firstChild.textContent  = fmtBytes(state.traffic_in).replace(/ b\/s$/, "");
    elTrafficIn.querySelector("span").textContent = "b/s";
    elTrafficOut.firstChild.textContent = fmtBytes(state.traffic_out).replace(/ b\/s$/, "");
    elTrafficOut.querySelector("span").textContent = "b/s";
    elFragReady.textContent = state.frag_ready;
    elFragNeed.textContent  = state.frag_needed;
    elFragBar.style.width   = `${Math.min(100, (state.frag_ready / state.frag_needed) * 100)}%`;
    elBalance.textContent   = state.balance;
    elConsump.textContent   = state.consumption.toFixed(2);
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

  // ── Local simulation (fallback) ──────────────────────────────────
  const tick = () => {
    state.uptime += 1;
    state.balance = Math.max(0, state.balance - 0.01);
    state.consumption = 0.05 + Math.random() * 0.2;

    // Smooth traffic with random walk
    const step = () => (Math.random() - 0.5) * 200;
    state.traffic_in  = Math.max(0, state.traffic_in  + step());
    state.traffic_out = Math.max(0, state.traffic_out + step());
    state.spark_in.push(state.traffic_in);
    state.spark_out.push(state.traffic_out);
    if (state.spark_in.length > 40)  state.spark_in.shift();
    if (state.spark_out.length > 40) state.spark_out.shift();

    // Random log entries
    if (Math.random() < 0.15) {
      const pool = [
        { kind: "info",    msg: "Découverte pair mDNS" },
        { kind: "success", msg: "Fragment transmis" },
        { kind: "info",    msg: "Ping pair distant" },
        { kind: "warn",    msg: "Latence élevée sur 1 pair" },
      ];
      state.log.push({ t: state.uptime, ...pool[Math.floor(Math.random() * pool.length)] });
      if (state.log.length > 30) state.log.shift();
    }
  };

  // ── Network fetch (with fallback) ────────────────────────────────
  let online = false;
  const fetchStatus = async () => {
    if (online) return; // already polling endpoint
    try {
      const res = await fetch(ENDPOINT, { cache: "no-store" });
      if (!res.ok) throw new Error(res.status);
      const data = await res.json();
      Object.assign(state, data);
      online = true;
    } catch {
      online = false; // stay in simulation
    }
  };

  // ── Init ─────────────────────────────────────────────────────────
  render();
  setInterval(() => { tick(); render(); }, 1000);
  setInterval(fetchStatus, POLL_MS);
  fetchStatus();
})();
