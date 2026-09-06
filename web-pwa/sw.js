self.addEventListener('install', e => { self.skipWaiting(); });
self.addEventListener('fetch', e => { e.respondWith(fetch(e.request)); });
self.addEventListener('activate', e => { e.waitUntil(self.clients.claim()); });