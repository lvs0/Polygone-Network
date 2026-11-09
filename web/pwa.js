/* ──────────────────────────────────────────────────────────────────
   Polygone — PWA
   Enregistre le service worker et propose l'installation uniquement
   quand le navigateur l'autorise (beforeinstallprompt).
   Silencieux sur les navigateurs qui ne le prennent pas en charge.
   ────────────────────────────────────────────────────────────────── */

(() => {
  "use strict";

  // ── Service worker ───────────────────────────────────────────────
  if ("serviceWorker" in navigator) {
    window.addEventListener("load", () => {
      navigator.serviceWorker.register("./sw.js").catch(() => {
        /* enregistrement impossible : la page reste utilisable en ligne */
      });
    });
  }

  // ── Installation ─────────────────────────────────────────────────
  let deferredPrompt = null;
  let button = null;

  function removeButton() {
    if (button && button.parentNode) button.parentNode.removeChild(button);
    button = null;
  }

  function showButton() {
    if (button) return;

    const style = document.createElement("style");
    style.textContent = [
      ".pwa-install{position:fixed;right:16px;bottom:16px;z-index:50;",
      "font:12px/1.4 'JetBrains Mono','Fira Code','Cascadia Code',monospace;",
      "color:#e2e2f0;background:#0c0c14;border:1px solid #1e1e30;",
      "border-radius:6px;padding:8px 12px;cursor:pointer;opacity:.85;",
      "transition:opacity 150ms ease,border-color 150ms ease}",
      ".pwa-install:hover{opacity:1;border-color:#7c3aed}",
      ".pwa-install:focus-visible{outline:2px solid #a78bfa;outline-offset:2px}"
    ].join("");
    document.head.appendChild(style);

    button = document.createElement("button");
    button.type = "button";
    button.className = "pwa-install";
    button.textContent = "Installer Polygone";
    button.addEventListener("click", async () => {
      if (!deferredPrompt) return;
      deferredPrompt.prompt();
      try {
        await deferredPrompt.userChoice;
      } catch (error) {
        /* choix indisponible : on retire simplement la proposition */
      }
      deferredPrompt = null;
      removeButton();
    });
    document.body.appendChild(button);
  }

  window.addEventListener("beforeinstallprompt", (event) => {
    event.preventDefault();
    deferredPrompt = event;
    showButton();
  });

  window.addEventListener("appinstalled", () => {
    deferredPrompt = null;
    removeButton();
  });
})();
