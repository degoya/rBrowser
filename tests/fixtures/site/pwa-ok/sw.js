// The service worker of the PWA fixture (RB-080-08): caches the app shell at install and serves
// it cache first, so the start URL opens offline.
const CACHE = 'pwa-ok-v1';
const SHELL = ['./', 'index.html', 'manifest.webmanifest', 'icons/icon-192.png'];

self.addEventListener('install', (event) => {
  event.waitUntil(caches.open(CACHE).then((cache) => cache.addAll(SHELL)));
  self.skipWaiting();
});

self.addEventListener('activate', (event) => {
  event.waitUntil(self.clients.claim());
});

self.addEventListener('fetch', (event) => {
  event.respondWith(caches.match(event.request).then((hit) => hit || fetch(event.request)));
});
