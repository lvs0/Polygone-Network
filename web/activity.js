/* ──────────────────────────────────────────────────────────────────
   Polygone — écran Activité (F1-EVT-01 + domaine usage)
   Lit les routes réelles de la gateway :
     GET /api/v1/events?since=&limit=  → flux borné à curseur
     GET /api/v1/usage                 → ledger actif + refus du daemon
   Le curseur avance à chaque page ; un gap (évincion) est affiché, jamais
   masqué. Aucune donnée n'est inventée : daemon absent = message explicite.
   ────────────────────────────────────────────────────────────────── */

(() => {
  "use strict";

  const POLL_MS = 3000;
  const MAX_ROWS = 200;
  const DASH = "—";
  const $ = (id) => document.getElementById(id);

  let cursor = 0;
  let seenIds = new Set();

  const shown = (value) => (Number.isFinite(value) ? String(value) : DASH);

  function fmtBytes(bytes) {
    if (!Number.isFinite(bytes)) return DASH;
    if (bytes >= 1024 ** 3) return `${(bytes / 1024 ** 3).toFixed(1)} Gio`;
    if (bytes >= 1024 ** 2) return `${(bytes / 1024 ** 2).toFixed(0)} Mio`;
    return `${bytes} o`;
  }

  function fmtTime(unix) {
    if (!Number.isFinite(unix)) return DASH;
    return new Date(unix * 1000).toLocaleTimeString();
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

  /* Résumé lisible d'un événement, selon son kind réel. Un kind inconnu
     s'affiche en JSON brut : on ne devine pas sa signification. */
  function summarize(event) {
    const d = event.data || {};
    switch (event.kind) {
      case "task_created":
      case "task_cancelled":
      case "task_resumed":
      case "task_expired":
        return `${d.id || "?"} → ${d.state || "?"}`;
      case "pair_request":
        return `demande de ${d.device_name || "?"}`;
      case "pair_accepted":
        return `${d.device_name || "?"} (${String(d.node_id || "?").slice(0, 12)}…)`;
      case "node_seen":
        return `${String(d.node_id || "?").slice(0, 12)}… vu`;
      case "ping_issued":
        return `challenge émis vers ${String(d.node_id || "?").slice(0, 12)}…`;
      case "rtt_measured":
        return `${String(d.node_id || "?").slice(0, 12)}… · ${shown(d.rtt_ms)} ms`;
      case "resources":
        return `seq ${shown(d.seq)} · tier ${d.tier || "?"} · RAM ${shown(d.ram_mb)} Mio`;
      case "daemon_up":
        return "daemon local joignable";
      case "daemon_absent":
        return "daemon local absent";
      case "gateway_up":
        return `gateway ${d.version || "?"} sur ${d.address || "?"}`;
      default:
        return JSON.stringify(d);
    }
  }

  function appendEvents(events) {
    const list = $("event-list");
    const empty = $("event-empty");
    // Les pages arrivent en ordre croissant ; on insère chaque nouveauté en
    // tête pour afficher le plus récent d'abord, sans doublon (curseur).
    for (const event of events) {
      if (seenIds.has(event.id)) continue;
      seenIds.add(event.id);
      if (empty) empty.remove();
      const li = document.createElement("li");
      li.textContent = `${fmtTime(event.at)} · #${event.id} · ${event.kind} · ${summarize(event)}`;
      list.prepend(li);
    }
    while (list.children.length > MAX_ROWS) {
      list.lastElementChild.remove();
    }
    // Le set de ids vus est borné comme l'affichage.
    if (seenIds.size > MAX_ROWS * 2) {
      seenIds = new Set([...seenIds].slice(-MAX_ROWS));
    }
  }

  function renderBanner(page) {
    const banner = $("activity-banner");
    banner.dataset.state = "active";
    $("activity-title").textContent = "Flux d'événements actif";
    $("activity-sub").textContent = `curseur ${cursor} · relecture toutes les ${POLL_MS / 1000} s`;
    $("ev-retained").textContent = shown(page.events ? page.capacity : null);
    $("ev-dropped").textContent = shown(page.dropped_total);
    $("ev-latest").textContent = shown(page.latest_id);
    $("gap-note").hidden = !page.gap;
  }

  function renderUsage(usage) {
    $("usage-ram").textContent = fmtBytes(usage.reserved_ram_bytes);
    $("usage-cpu").textContent = shown(usage.reserved_cpu_cores);
    $("usage-storage").textContent = fmtBytes(usage.reserved_storage_bytes);
    const grants = Array.isArray(usage.grants) ? usage.grants : [];
    $("usage-count").textContent = String(grants.length);

    const list = $("usage-grants");
    list.innerHTML = "";
    for (const grant of grants) {
      const li = document.createElement("li");
      const expires = grant.expires_at
        ? `expire ${fmtTime(grant.expires_at)}`
        : "sans échéance publiée";
      li.textContent = `${grant.allocation_id} · ${shown(grant.cores)} cœur(s) · ${fmtBytes(grant.ram_bytes)} · ${expires}`;
      list.appendChild(li);
    }

    const refusals = $("usage-refusals");
    refusals.innerHTML = "";
    const entries = Object.entries(usage.refusals || {}).sort();
    if (!entries.length) {
      const li = document.createElement("li");
      li.textContent = "aucun refus enregistré";
      refusals.appendChild(li);
    }
    for (const [reason, count] of entries) {
      const li = document.createElement("li");
      li.textContent = `${reason} : ${count}`;
      refusals.appendChild(li);
    }
    const note = $("usage-note");
    note.textContent = "ledger du daemon local, relayé à l'identique";
    delete note.dataset.error;
  }

  function setUsageError(message) {
    const note = $("usage-note");
    note.textContent = message;
    note.dataset.error = "true";
    for (const id of ["usage-ram", "usage-cpu", "usage-storage", "usage-count"]) {
      $(id).textContent = DASH;
    }
    $("usage-grants").innerHTML = "";
  }

  async function refresh() {
    try {
      const page = await fetchRoute(`/api/v1/events?since=${cursor}&limit=100`);
      cursor = Number.isFinite(page.next_since) ? page.next_since : cursor;
      appendEvents(Array.isArray(page.events) ? page.events : []);
      renderBanner(page);
    } catch (error) {
      const banner = $("activity-banner");
      banner.dataset.state = "error";
      $("activity-title").textContent = "Gateway injoignable";
      $("activity-sub").textContent = `${error.message} · aucun événement inventé`;
    }
    await fetchRoute("/api/v1/usage")
      .then(renderUsage)
      .catch((e) => setUsageError(e.message));
  }

  refresh();
  setInterval(refresh, POLL_MS);
})();