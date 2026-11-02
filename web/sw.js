/* ──────────────────────────────────────────────────────────────────
   Polygone — service worker
   Coque applicative en cache-first ; tout le reste en réseau d'abord.
   Les réponses de l'API ne sont jamais mises en cache.
   Incrémenter VERSION pour publier une nouvelle coque.
   ────────────────────────────────────────────────────────────────── */

const VERSION = "polygone-v1";
const CACHE = `polygone-shell-${VERSION}`;

// Coque applicative : les fichiers nécessaires pour afficher l'interface
// hors-ligne. Tous les chemins sont relatifs à la portée du service worker.
const SHELL = [
  "./index.html",
  "./landing.html",
  "./offline.html",
  "./app.js",
  "./style.css",
  "./manifest.webmanifest",
  "./icon.svg"
];

self.addEventListener("install", (event) => {
  event.waitUntil(
    (async () => {
      const cache = await caches.open(CACHE);
      // Un fichier manquant ne doit pas faire échouer toute l'installation.
      await Promise.all(
        SHELL.map((url) =>
          cache.add(url).catch(() => {
            /* fichier absent : ignoré, la coque reste utilisable */
          })
        )
      );
      await self.skipWaiting();
    })()
  );
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    (async () => {
      const names = await caches.keys();
      await Promise.all(
        names
          .filter((name) => name.startsWith("polygone-shell-") && name !== CACHE)
          .map((name) => caches.delete(name))
      );
      await self.clients.claim();
    })()
  );
});

function isApiRequest(url) {
  return url.pathname.startsWith("/api/") || url.pathname.includes("/api/v1/");
}

function isShellAsset(url) {
  return SHELL.some((entry) => url.pathname.endsWith(entry.replace("./", "/")));
}

self.addEventListener("fetch", (event) => {
  const request = event.request;
  if (request.method !== "GET") return;

  const url = new URL(request.url);

  // Hors origine : on ne s'interpose pas.
  if (url.origin !== self.location.origin) return;

  // L'API n'est jamais servie depuis le cache.
  if (isApiRequest(url)) return;

  // Navigation : réseau d'abord, cache ensuite, page hors-ligne en dernier.
  if (request.mode === "navigate") {
    event.respondWith(
      (async () => {
        try {
          return await fetch(request);
        } catch (error) {
          const cache = await caches.open(CACHE);
          const cached =
            (await cache.match(request)) ||
            (await cache.match("./index.html"));
          return cached || (await cache.match("./offline.html"));
        }
      })()
    );
    return;
  }

  // Ressources de la coque : cache d'abord, puis réseau (et mise en cache).
  if (isShellAsset(url)) {
    event.respondWith(
      (async () => {
        const cache = await caches.open(CACHE);
        const cached = await cache.match(request);
        if (cached) return cached;
        try {
          const response = await fetch(request);
          if (response.ok) cache.put(request, response.clone());
          return response;
        } catch (error) {
          return new Response("", { status: 504, statusText: "Hors-ligne" });
        }
      })()
    );
    return;
  }

  // Reste des ressources statiques de même origine : réseau d'abord.
  event.respondWith(
    (async () => {
      const cache = await caches.open(CACHE);
      try {
        const response = await fetch(request);
        if (response.ok) cache.put(request, response.clone());
        return response;
      } catch (error) {
        const cached = await cache.match(request);
        if (cached) return cached;
        return new Response("", { status: 504, statusText: "Hors-ligne" });
      }
    })()
  );
});
