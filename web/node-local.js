/* ──────────────────────────────────────────────────────────────────
   Polygone — panneaux locaux de la console (F1-API-01)
   Lit les routes réelles de la gateway :
     GET /api/v1/identity   → identité publique (jamais les clés secrètes)
     GET /api/v1/daemon     → version, plateforme, tier, protocole, capacités
     GET /api/v1/resources  → status_response du daemon, relayé tel quel
     GET /api/v1/settings   → configuration lecture seule (file ou defaults)
   Règles : null ≠ 0, une capacité non publiée reste « non publié »,
   une erreur s'affiche avec son code et son action. Rien n'est simulé.
   ────────────────────────────────────────────────────────────────── */

(() => {
  "use strict";

  const POLL_MS = 5000;
  const DASH = "—";
  const $ = (id) => document.getElementById(id);

  const shown = (value) => (Number.isFinite(value) ? String(value) : DASH);

  function hexFingerprint(hex) {
    if (typeof hex !== "string" || hex.length === 0) return DASH;
    return `${hex.slice(0, 16)}…${hex.slice(-8)}`;
  }

  function fmtBytes(bytes) {
    if (!Number.isFinite(bytes)) return DASH;
    if (bytes >= 1024 ** 3) return `${(bytes / 1024 ** 3).toFixed(1)} Gio`;
    if (bytes >= 1024 ** 2) return `${(bytes / 1024 ** 2).toFixed(0)} Mio`;
    return `${bytes} o`;
  }

  function fmtUptime(secs) {
    if (!Number.isFinite(secs)) return DASH;
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    return h > 0 ? `${h} h ${m} min` : `${m} min`;
  }

  /* Chaque route est indépendante : une gateway ou un daemon partiel
     n'efface pas les autres panneaux. L'erreur est lue dans le corps JSON
     (code + hint) et affichée franchement. */
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

  function setError(noteId, message) {
    const el = $(noteId);
    if (el) {
      el.textContent = message;
      el.dataset.error = "true";
    }
  }

  function clearError(noteId, message) {
    const el = $(noteId);
    if (el) {
      el.textContent = message;
      delete el.dataset.error;
    }
  }

  function renderIdentity(identity) {
    $("id-pseudo").textContent = identity.pseudo || DASH;
    const nodeId = identity.node_id;
    $("id-node-id").textContent = nodeId ? hexFingerprint(nodeId) : "non dérivable";
    $("id-node-id").title = nodeId || "clé publique illisible";
    $("id-sign-pk").textContent = hexFingerprint(identity.sign_pk_hex);
    $("id-sign-pk").title = identity.sign_pk_hex || "";
    $("id-kem-pk").textContent = hexFingerprint(identity.kem_pk_hex);
    $("id-kem-pk").title = identity.kem_pk_hex || "";
    $("id-rotated").textContent =
      identity.rotated_at_unix == null
        ? "jamais"
        : new Date(identity.rotated_at_unix * 1000).toLocaleString();
    const revoked = Array.isArray(identity.revoked_sign_pk_hex)
      ? identity.revoked_sign_pk_hex.length
      : null;
    $("id-revoked").textContent = shown(revoked);
    clearError("id-note", "identité publique locale — les clés secrètes ne sont jamais servies");
  }

  function renderDaemon(hello) {
    $("daemon-version").textContent = hello.daemon_version || DASH;
    $("daemon-platform").textContent = hello.platform || DASH;
    $("daemon-tier").textContent = hello.tier || DASH;
    $("daemon-protocol").textContent = shown(hello.protocol_version);
    $("daemon-caps").textContent = Array.isArray(hello.capabilities)
      ? hello.capabilities.join(", ")
      : DASH;
    clearError("daemon-note", "hello du daemon, relayé par la gateway");
  }

  function renderResources(status) {
    $("res-tier").textContent = status.tier || DASH;
    $("res-seq").textContent = shown(status.seq);
    $("res-ram").textContent = Number.isFinite(status.ram_mb)
      ? `${(status.ram_mb / 1024).toFixed(1)} Gio`
      : DASH;
    const cap = status.ram_capacity;
    $("res-ram-measured").textContent = cap ? fmtBytes(cap.measured) : DASH;
    $("res-ram-reservable").textContent = cap ? fmtBytes(cap.reservable) : DASH;
    $("res-ram-enforced").textContent = cap ? fmtBytes(cap.enforced) : DASH;
    $("res-cpu").textContent = shown(status.cpu_cores);
    $("res-bandwidth").textContent =
      status.bandwidth_mbps == null ? "non mesuré" : `${status.bandwidth_mbps} Mb/s`;
    $("res-gpu").textContent =
      status.gpu_free_vram_mb == null
        ? "non publié"
        : `${status.gpu_free_vram_mb} Mio`;
    const storage = status.storage_capacity;
    $("res-storage").textContent = storage ? fmtBytes(storage.reservable) : "non publié";
    $("res-shrinking").textContent =
      status.shrinking === true ? "oui (repli)" : status.shrinking === false ? "non" : DASH;
    $("res-uptime").textContent = fmtUptime(status.uptime_secs);
    clearError("res-note", "mesures du daemon local — relais identique au socket");
  }

  function renderSettings(settings) {
    const config = settings.config || {};
    $("set-source").textContent =
      settings.source === "file" ? "fichier daemon.toml" : "defaults produit (fichier absent)";
    $("set-tier").textContent = config.tier || DASH;
    $("set-memory-limit").textContent =
      config.memory_limit_enabled === true ? "activée" : config.memory_limit_enabled === false ? "désactivée" : DASH;
    $("set-bandwidth").textContent =
      config.bandwidth_shaping === true ? "activé" : config.bandwidth_shaping === false ? "désactivé" : DASH;
    clearError("set-note", "lecture seule — les modifications passent par la CLI (polygone)");
  }

  async function refresh() {
    await Promise.all([
      fetchRoute("/api/v1/identity")
        .then(renderIdentity)
        .catch((e) => setError("id-note", e.message)),
      fetchRoute("/api/v1/daemon")
        .then(renderDaemon)
        .catch((e) => setError("daemon-note", e.message)),
      fetchRoute("/api/v1/resources")
        .then(renderResources)
        .catch((e) => setError("res-note", e.message)),
      fetchRoute("/api/v1/settings")
        .then(renderSettings)
        .catch((e) => setError("set-note", e.message)),
    ]);
  }

  refresh();
  setInterval(refresh, POLL_MS);
})();