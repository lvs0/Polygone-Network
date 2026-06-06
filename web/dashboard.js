/* ──────────────────────────────────────────────────────────────────
   Polygone — node dashboard logic
   Features: WebSocket live updates, mini charts, theme toggle,
   improved sparklines, module progress bars, animated transitions.
   ────────────────────────────────────────────────────────────────── */

(() => {
  "use strict";

  const ENDPOINT = "/api/status";
  const WS_ENDPOINT = `ws://${location.host}/ws`;
  const POLL_MS = 3000;
  const SPARK_MAX = 40;

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
      { name: "Brain", icon: "🧠", status: "soon",    label: "Soon", progress: 35 },
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
    wsConnected: false,
    theme: localStorage.getItem("polygone-theme") || "cyber",
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

  // ── Theme management ────────────────────────────────────────────
  const applyTheme = (theme) => {
    state.theme = theme;
    document.body.className = theme === "cyber" ? "" : `theme-${theme}`;
    localStorage.setItem("polygone-theme", theme);
    const toggle = document.querySelector(".theme-toggle");
    if (toggle) {
      const icons = { cyber: "🔮", solarized: "☀️", dracula: "🧛" };
      toggle.querySelector(".theme-toggle__icon").textContent = icons[theme] || "🔮";
      toggle.querySelector(".theme-toggle__label").textContent =
        theme === "cyber" ? "Cyber" : theme === "solarized" ? "Solarized" : "Dracula";
    }
  };

  const cycleTheme = () => {
    const themes = ["cyber", "solarized", "dracula"];
    const idx = themes.indexOf(state.theme);
    applyTheme(themes[(idx + 1) % themes.length]);
  };

  // ── Render functions ─────────────────────────────────────────────
  const renderBanner = () => {
    elUptime.textContent = fmtUptime(state.uptime);
    elPeers.textContent  = state.peers;
  };

  const renderStats = () => {
    const inText = fmtBytes(state.traffic_in);
    const outText = fmtBytes(state.traffic_out);

    // Parse the formatted text to split number and unit
    const inParts = inText.split(" ");
    const outParts = outText.split(" ");

    elTrafficIn.firstChild.textContent = inParts[0] + " ";
    elTrafficIn.querySelector("span").textContent = inParts.slice(1).join(" ");
    elTrafficOut.firstChild.textContent = outParts[0] + " ";
    elTrafficOut.querySelector("span").textContent = outParts.slice(1).join(" ");
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

      // Add progress bar for "soon" modules
      if (m.status === "soon" && m.progress != null) {
        const progressEl = document.createElement("div");
        progressEl.className = "module__progress";
        progressEl.innerHTML = `
          <div class="module__progress-bar">
            <div class="module__progress-fill" style="width: ${m.progress}%"></div>
          </div>
        `;
        node.appendChild(progressEl);
      }

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

  // ── Mini chart renderer ──────────────────────────────────────────
  const renderMiniChart = (canvasId, data, color) => {
    const canvas = document.getElementById(canvasId);
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    const w = canvas.width = canvas.offsetWidth * 2;
    const h = canvas.height = canvas.offsetHeight * 2;
    ctx.clearRect(0, 0, w, h);

    if (data.length < 2) return;

    const max = Math.max(...data, 1);
    const step = w / (data.length - 1);

    // Draw fill gradient
    const gradient = ctx.createLinearGradient(0, 0, 0, h);
    gradient.addColorStop(0, color + "40");
    gradient.addColorStop(1, color + "05");

    ctx.beginPath();
    ctx.moveTo(0, h);
    data.forEach((v, i) => {
      const x = i * step;
      const y = h - (v / max) * h * 0.85 - 2;
      if (i === 0) ctx.lineTo(x, y);
      else ctx.lineTo(x, y);
    });
    ctx.lineTo(w, h);
    ctx.closePath();
    ctx.fillStyle = gradient;
    ctx.fill();

    // Draw line
    ctx.beginPath();
    data.forEach((v, i) => {
      const x = i * step;
      const y = h - (v / max) * h * 0.85 - 2;
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    });
    ctx.strokeStyle = color;
    ctx.lineWidth = 2;
    ctx.lineJoin = "round";
    ctx.stroke();

    // Draw latest point
    if (data.length > 0) {
      const lastX = (data.length - 1) * step;
      const lastY = h - (data[data.length - 1] / max) * h * 0.85 - 2;
      ctx.beginPath();
      ctx.arc(lastX, lastY, 4, 0, Math.PI * 2);
      ctx.fillStyle = color;
      ctx.fill();
      ctx.beginPath();
      ctx.arc(lastX, lastY, 8, 0, Math.PI * 2);
      ctx.fillStyle = color + "30";
      ctx.fill();
    }
  };

  const render = () => {
    renderBanner();
    renderStats();
    renderSpark();
    renderModules();
    renderLog();

    // Render mini charts
    renderMiniChart("chart-in", state.spark_in, "#22c55e");
    renderMiniChart("chart-out", state.spark_out, "#22d3ee");
  };

  // ── WebSocket connection ─────────────────────────────────────────
  let ws = null;
  let wsReconnectTimer = null;

  const connectWebSocket = () => {
    try {
      ws = new WebSocket(WS_ENDPOINT);

      ws.onopen = () => {
        state.wsConnected = true;
        updateWsStatus();
        console.log("[ws] connected");
      };

      ws.onmessage = (event) => {
        try {
          const data = JSON.parse(event.data);
          if (data.type === "status") {
            Object.assign(state, data.payload);
            render();
          } else if (data.type === "log") {
            state.log.push(data.payload);
            if (state.log.length > 30) state.log.shift();
            renderLog();
          }
        } catch (e) {
          console.warn("[ws] parse error:", e);
        }
      };

      ws.onclose = () => {
        state.wsConnected = false;
        updateWsStatus();
        console.log("[ws] disconnected, reconnecting in 5s...");
        wsReconnectTimer = setTimeout(connectWebSocket, 5000);
      };

      ws.onerror = () => {
        state.wsConnected = false;
        updateWsStatus();
      };
    } catch (e) {
      console.warn("[ws] connection failed:", e);
      state.wsConnected = false;
      updateWsStatus();
    }
  };

  const updateWsStatus = () => {
    const el = document.querySelector(".ws-status");
    if (el) {
      el.dataset.connected = state.wsConnected;
      el.querySelector(".ws-status__label").textContent =
        state.wsConnected ? "WebSocket" : "Polling";
    }
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
    if (state.spark_in.length > SPARK_MAX)  state.spark_in.shift();
    if (state.spark_out.length > SPARK_MAX) state.spark_out.shift();

    // Animate Brain progress
    const brain = state.modules.find(m => m.name === "Brain");
    if (brain && brain.progress != null && brain.progress < 100) {
      brain.progress = Math.min(100, brain.progress + Math.random() * 0.3);
    }

    // Random log entries
    if (Math.random() < 0.15) {
      const pool = [
        { kind: "info",    msg: "Découverte pair mDNS" },
        { kind: "success", msg: "Fragment transmis" },
        { kind: "info",    msg: "Ping pair distant" },
        { kind: "warn",    msg: "Latence élevée sur 1 pair" },
        { kind: "success", msg: "Shamir reconstruction réussie" },
        { kind: "info",    msg: "KDF BLAKE3 — clé dérivée" },
      ];
      state.log.push({ t: state.uptime, ...pool[Math.floor(Math.random() * pool.length)] });
      if (state.log.length > 30) state.log.shift();
    }
  };

  // ── Network fetch (with fallback) ────────────────────────────────
  let online = false;
  const fetchStatus = async () => {
    if (online || state.wsConnected) return;
    try {
      const res = await fetch(ENDPOINT, { cache: "no-store" });
      if (!res.ok) throw new Error(res.status);
      const data = await res.json();
      Object.assign(state, data);
      online = true;
    } catch {
      online = false;
    }
  };

  // ── Init ─────────────────────────────────────────────────────────
  applyTheme(state.theme);

  // Theme toggle click handler
  const themeToggle = document.querySelector(".theme-toggle");
  if (themeToggle) {
    themeToggle.addEventListener("click", cycleTheme);
  }

  render();
  setInterval(() => { tick(); render(); }, 1000);
  setInterval(fetchStatus, POLL_MS);
  fetchStatus();

  // Try WebSocket, fall back to polling
  connectWebSocket();
})();
