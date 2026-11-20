/* Polygone — écran Réseau / Appareils (données réelles).
 *
 * Lit la gateway locale : /api/v1/network/status, /api/v1/nodes,
 * /api/v1/events. Révoque un appareil via POST /api/v1/nodes/{id}/revoke.
 * Aucune donnée fictive : pas de pair codé en dur, pas de CPU/RAM inventé,
 * pas d'IP aléatoire. null ≠ 0 (« — »), une erreur s'affiche, rien n'est simulé.
 */
(() => {
  "use strict";

  const POLL_MS = 5000;
  const DASH = "—";
  const $ = (id) => document.getElementById(id);
  const shown = (value) => (Number.isFinite(value) ? String(value) : DASH);
  const revoking = new Set();

  function hexFingerprint(hex) {
    if (typeof hex !== "string" || hex.length === 0) return DASH;
    return `${hex.slice(0, 16)}…${hex.slice(-8)}`;
  }

  async function fetchRoute(url) {
    const response = await fetch(url, {
      cache: "no-store",
      headers: { Accept: "application/json" },
    });
    const body = await response.json().catch(() => null);
    if (!response.ok) {
      const code = body && body.error ? body.error : `HTTP ${response.status}`;
      const hint = body && body.hint ? ` — ${body.hint}` : "";
      throw new Error(`${code}${hint}`);
    }
    return body;
  }

  function setBanner(state, title, sub) {
    $("banner").dataset.state = state;
    $("banner-title").textContent = title;
    $("banner-sub").textContent = sub;
  }

  function renderStatus(status) {
    $("net-nodes").textContent = shown(status.nodes);
    $("net-active").textContent = shown(status.active);
    $("net-latency").textContent =
      status.latency_ms_median == null
        ? "aucune mesure"
        : `${status.latency_ms_median} ms`;
    if (Number.isFinite(status.nodes) && status.nodes > 0) {
      setBanner(
        "active",
        "Réseau actif",
        `${shown(status.active)} appareil(s) actif(s) sur ${status.nodes} appairé(s)`,
      );
    } else {
      setBanner("unknown", "Aucun appareil appairé", "appairez un appareil pour le voir ici");
    }
  }

  function renderDevices(data) {
    const list = $("devices");
    const template = $("device-template");
    const state = $("devices-state");
    const note = $("devices-note");
    list.innerHTML = "";
    delete note.dataset.error;
    const nodes = Array.isArray(data.nodes) ? data.nodes : [];
    if (nodes.length === 0) {
      state.hidden = false;
      state.textContent =
        "Aucun appareil appairé pour l'instant. Lancez un appairage (pair/request + pair/accept) ou utilisez la CLI polygone.";
      return;
    }
    state.hidden = true;
    nodes.forEach((node) => {
      const el = template.content.firstElementChild.cloneNode(true);
      el.querySelector(".device__dot").classList.add(
        node.active ? "device__dot--on" : "device__dot--off",
      );
      el.querySelector(".device__name").textContent =
        node.device_name || "appareil sans nom";
      const idEl = el.querySelector(".device__id");
      idEl.textContent = hexFingerprint(node.node_id);
      idEl.title = node.node_id || "";
      el.querySelector(".device__meta").textContent = node.active
        ? "actif (vu il y a moins de 5 min)"
        : "inactif";
      el.querySelector(".device__latency").textContent =
        node.latency_ms == null ? "latence —" : `${node.latency_ms} ms`;
      el.querySelector(".device__services").textContent =
        Array.isArray(node.services) && node.services.length > 0
          ? node.services.join(", ")
          : "aucun service";
      const button = el.querySelector(".device__revoke");
      button.disabled = revoking.has(node.node_id);
      button.addEventListener("click", () => revoke(node, button));
      list.appendChild(el);
    });
  }

  async function revoke(node, button) {
    const name = node.device_name || hexFingerprint(node.node_id);
    const ok = window.confirm(
      `Révoquer « ${name} » ? Cet appareil ne sera plus approuvé sur cet hôte.`,
    );
    if (!ok) return;
    button.disabled = true;
    revoking.add(node.node_id);
    const note = $("devices-note");
    try {
      const response = await fetch(
        `/api/v1/nodes/${encodeURIComponent(node.node_id)}/revoke`,
        { method: "POST", cache: "no-store", headers: { Accept: "application/json" } },
      );
      const body = await response.json().catch(() => null);
      if (!response.ok) {
        const code = body && body.error ? body.error : `HTTP ${response.status}`;
        throw new Error(code);
      }
      note.textContent = `« ${name} » révoqué : ancre locale retirée.`;
      delete note.dataset.error;
      await refresh();
    } catch (error) {
      note.textContent = `Échec de la révocation : ${error.message}`;
      note.dataset.error = "true";
      button.disabled = false;
    } finally {
      revoking.delete(node.node_id);
    }
  }

  function renderEvents(page) {
    const list = $("net-events");
    list.innerHTML = "";
    const events = Array.isArray(page.events) ? page.events : [];
    const kinds = ["pair_request", "pair_accepted", "node_revoked", "heartbeat"];
    const relevant = events.filter((event) => kinds.includes(event.kind));
    if (relevant.length === 0) {
      const li = document.createElement("li");
      li.className = "log__entry";
      li.textContent = "Aucun événement réseau récent.";
      list.appendChild(li);
      return;
    }
    relevant
      .slice(-12)
      .reverse()
      .forEach((event) => {
        const li = document.createElement("li");
        li.className = "log__entry";
        const time = document.createElement("span");
        time.className = "log__time";
        time.textContent = new Date(event.at * 1000).toLocaleTimeString();
        const who = event.data.device_name || hexFingerprint(event.data.node_id);
        const label =
          event.kind === "pair_accepted"
            ? `appairé : ${who}`
            : event.kind === "node_revoked"
              ? `révoqué : ${who}`
              : event.kind === "pair_request"
                ? `demande d'appairage : ${event.data.device_name || ""}`
                : event.kind;
        li.appendChild(time);
        li.appendChild(document.createTextNode(` ${label}`));
        list.appendChild(li);
      });
  }

  async function refresh() {
    await Promise.all([
      fetchRoute("/api/v1/network/status")
        .then(renderStatus)
        .catch((error) => setBanner("error", "Gateway injoignable", error.message)),
      fetchRoute("/api/v1/nodes")
        .then(renderDevices)
        .catch((error) => {
          $("devices").innerHTML = "";
          const state = $("devices-state");
          state.hidden = false;
          state.textContent = `Gateway injoignable : ${error.message}`;
          const note = $("devices-note");
          note.textContent = `erreur : ${error.message}`;
          note.dataset.error = "true";
        }),
      fetchRoute("/api/v1/events?limit=50")
        .then(renderEvents)
        .catch(() => {
          /* les événements sont secondaires ; le panneau appareils prime */
        }),
    ]);
  }

  refresh();
  setInterval(refresh, POLL_MS);
})();