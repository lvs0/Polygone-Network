/* ──────────────────────────────────────────────────────────────────
   Polygone — écran Compute (domaine tâches, F1-API-01)
   Lit et écrit les routes réelles de la gateway :
     POST /api/v1/tasks            → création (echo, blake3)
     GET  /api/v1/tasks?limit=     → liste réelle, paginée
     GET  /api/v1/tasks/:id        → détail + résultat signé
     POST /api/v1/tasks/:id/cancel → annulation
   Un résultat n'est affiché que s'il existe vraiment dans la réponse ;
   la signature est montrée (producteur, empreinte), pas vérifiée dans le
   navigateur — la vérification ML-DSA-65 complète reste à la CLI.
   ────────────────────────────────────────────────────────────────── */

(() => {
  "use strict";

  const POLL_MS = 3000;
  const DASH = "—";
  const $ = (id) => document.getElementById(id);

  const shown = (value) => (Number.isFinite(value) ? String(value) : DASH);

  function hexToText(hex) {
    if (typeof hex !== "string" || hex.length === 0 || hex.length % 2 !== 0) return null;
    const bytes = new Uint8Array(hex.match(/.{2}/g).map((b) => parseInt(b, 16)));
    // Affichage texte seulement si tout est imprimable ; sinon hex brut.
    if (bytes.every((b) => b === 9 || b === 10 || b === 13 || (b >= 32 && b < 127))) {
      return new TextDecoder().decode(bytes);
    }
    return null;
  }

  function textToHex(text) {
    return [...new TextEncoder().encode(text)]
      .map((b) => b.toString(16).padStart(2, "0"))
      .join("");
  }

  async function fetchRoute(url, options) {
    const response = await fetch(url, {
      cache: "no-store",
      headers: { Accept: "application/json", ...(options && options.body ? { "Content-Type": "application/json" } : {}) },
      ...options,
    });
    const body = await response.json().catch(() => null);
    if (!response.ok) {
      const code = body && body.error ? body.error : `HTTP ${response.status}`;
      const message = body && body.message ? ` — ${body.message}` : "";
      throw new Error(`${code}${message}`);
    }
    return body;
  }

  function setFormNote(message, isError) {
    const note = $("form-note");
    note.textContent = message;
    if (isError) note.dataset.error = "true";
    else delete note.dataset.error;
  }

  function renderTask(task, executorAttached) {
    const li = document.createElement("li");
    const spec = task.spec || {};
    const state = task.state || "inconnu";
    const id = spec.id || "?";

    let line = `${id} · ${spec.kind || "?"} · ${state}`;
    if (task.last_error) line += ` · erreur : ${task.last_error}`;

    const result = task.result;
    if (result) {
      const outputHex = typeof result.output === "string" ? result.output : "";
      const text = hexToText(outputHex);
      const output = text !== null ? `« ${text} »` : `${outputHex.slice(0, 64)}${outputHex.length > 64 ? "…" : ""}`;
      line += ` · sortie : ${output}`;
      line += ` · signé par ${String(result.producer || "?").slice(0, 12)}…`;
      if (result.output_hash) line += ` · blake3 ${result.output_hash.slice(0, 16)}…`;
    } else if (state === "succeeded") {
      line += " · résultat absent de la réponse";
    }

    li.textContent = line;

    // Annulation seulement pour un état non terminal réel.
    if (["created", "validated", "published", "assigned", "accepted", "running"].includes(state)) {
      const button = document.createElement("button");
      button.textContent = "annuler";
      button.style.marginLeft = "8px";
      button.addEventListener("click", async () => {
        button.disabled = true;
        try {
          await fetchRoute(`/api/v1/tasks/${encodeURIComponent(id)}/cancel`, { method: "POST", body: "{}" });
          setFormNote(`tâche ${id} annulée`, false);
          refresh();
        } catch (error) {
          setFormNote(`annulation refusée : ${error.message}`, true);
          button.disabled = false;
        }
      });
      li.appendChild(button);
    }
    return li;
  }

  async function refresh() {
    try {
      const page = await fetchRoute("/api/v1/tasks?limit=20");
      const banner = $("compute-banner");
      banner.dataset.state = "active";
      $("compute-title").textContent = "Compute local joignable";
      const attached = page.executor_attached === true;
      $("executor-state").textContent = attached ? "attaché (local)" : "absent";
      $("compute-sub").textContent = attached
        ? "les tâches publiées sont exécutées et signées localement"
        : "sans exécuteur : les tâches restent published, aucun calcul n'a lieu";
      $("task-total").textContent = shown(page.total);

      const list = $("task-list");
      list.innerHTML = "";
      const tasks = Array.isArray(page.tasks) ? page.tasks : [];
      if (!tasks.length) {
        const li = document.createElement("li");
        li.textContent = "aucune tâche";
        list.appendChild(li);
      }
      for (const task of tasks) {
        list.appendChild(renderTask(task, attached));
      }
    } catch (error) {
      const banner = $("compute-banner");
      banner.dataset.state = "error";
      $("compute-title").textContent = "Gateway injoignable";
      $("compute-sub").textContent = `${error.message} · aucune tâche inventée`;
      $("executor-state").textContent = DASH;
      $("task-total").textContent = DASH;
    }
  }

  $("task-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    const button = $("task-submit");
    button.disabled = true;
    try {
      const body = {
        owner: "console-web",
        kind: $("task-kind").value,
        input_hex: textToHex($("task-input").value),
        deadline_in_seconds: Number($("task-deadline").value) || 300,
      };
      const created = await fetchRoute("/api/v1/tasks", {
        method: "POST",
        body: JSON.stringify(body),
      });
      const task = created.task || {};
      const state = task.state || "published";
      setFormNote(
        `tâche ${task.spec && task.spec.id ? task.spec.id : "?"} créée · état : ${state}` +
          (created.executor_attached ? " · exécutée localement" : " · aucun exécuteur attaché"),
        false
      );
      refresh();
    } catch (error) {
      setFormNote(`création refusée : ${error.message}`, true);
    } finally {
      button.disabled = false;
    }
  });

  refresh();
  setInterval(refresh, POLL_MS);
})();