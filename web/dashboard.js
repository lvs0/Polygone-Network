/* ──────────────────────────────────────────────────────────────────
   Polygone — console locale
   Lit les routes réelles de la gateway :
     GET /api/v1/network/status  → nodes, active, latency_ms_median, uptime
     GET /api/v1/services        → état déclaré des services
   Quand la gateway ne répond pas, toutes les mesures restent « — ».
   Aucune valeur plausible n'est inventée : une console qui ment est pire
   qu'une console qui dit « je ne sais pas ».
   ────────────────────────────────────────────────────────────────── */

(() => {
  "use strict";

  const STATUS_URL = "/api/v1/network/status";
  const SERVICES_URL = "/api/v1/services";
  const POLL_MS = 3000;
  const DASH = "—";

  const state = {
    online: false,
    status: null,
    services: [],
  };

  const $ = (id) => document.getElementById(id);
  const banner = document.querySelector(".banner");
  const elTitle = $("banner-title");
  const elSub = $("banner-sub");
  const elNodes = $("nodes");
  const elActive = $("active");
  const elLatency = $("latency");
  const elStatActive = $("stat-active");
  const elStatNodes = $("stat-nodes");
  const elStatLatency = $("stat-latency");
  const elLatencyUnit = $("stat-latency-unit");
  const elLatencyNote = $("latency-note");
  const elModules = $("modules");
  const moduleTpl = document.getElementById("module-template");

  const shown = (value) => (Number.isFinite(value) ? String(value) : DASH);

  function renderBanner() {
    banner.dataset.state = state.online ? "active" : "error";
    elTitle.textContent = state.online ? "Gateway joignable" : "Gateway injoignable";
    elSub.textContent = state.online
      ? `${location.host} · mesures relues toutes les ${POLL_MS / 1000} s`
      : `${location.host} · aucune mesure inventée`;
  }

  function renderStatus() {
    const status = state.status;
    const nodes = status && Number.isFinite(status.nodes) ? status.nodes : null;
    const active = status && Number.isFinite(status.active) ? status.active : null;
    const latency =
      status && Number.isFinite(status.latency_ms_median)
        ? status.latency_ms_median
        : null;

    elNodes.textContent = shown(nodes);
    elActive.textContent = shown(active);
    elLatency.textContent = latency === null ? DASH : `${latency} ms`;
    elStatActive.textContent = shown(active);
    elStatNodes.textContent = shown(nodes);
    elStatLatency.textContent = latency === null ? DASH : String(latency);
    elLatencyUnit.textContent = latency === null ? "" : "ms";
    elLatencyNote.textContent =
      latency === null
        ? "aucune mesure signée ping/pong"
        : "médiane des latences réellement mesurées";
  }

  function serviceLabel(entry) {
    switch (entry.state) {
      case "live":
        return { status: "running", label: "live" };
      case "partial":
        return { status: "soon", label: "partiel" };
      default:
        return { status: "off", label: String(entry.state || "inconnu") };
    }
  }

  function renderServices() {
    elModules.innerHTML = "";
    if (!state.services.length) {
      elModules.dataset.empty = "true";
      elModules.textContent = state.online
        ? "aucun service annoncé"
        : DASH;
      return;
    }
    delete elModules.dataset.empty;
    for (const entry of state.services) {
      const node = moduleTpl.content.firstElementChild.cloneNode(true);
      const mapped = serviceLabel(entry);
      node.dataset.status = mapped.status;
      node.querySelector(".module__icon").textContent = "◈";
      node.querySelector(".module__name").textContent = entry.name;
      node.querySelector(".module__status").textContent = mapped.label;
      node.title = `depuis ${entry.since || "inconnu"}`;
      elModules.appendChild(node);
    }
  }

  function render() {
    renderBanner();
    renderStatus();
    renderServices();
  }

  async function fetchJson(url) {
    const response = await fetch(url, {
      cache: "no-store",
      headers: { Accept: "application/json" },
    });
    if (!response.ok) throw new Error(`${url}: HTTP ${response.status}`);
    return response.json();
  }

  async function refresh() {
    try {
      const [status, services] = await Promise.all([
        fetchJson(STATUS_URL),
        fetchJson(SERVICES_URL),
      ]);
      state.online = true;
      state.status = status;
      state.services = Array.isArray(services.services) ? services.services : [];
    } catch (error) {
      state.online = false;
      state.status = null;
      state.services = [];
    }
    render();
  }

  render();
  refresh();
  setInterval(refresh, POLL_MS);
})();
