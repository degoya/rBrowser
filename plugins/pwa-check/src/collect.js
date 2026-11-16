// The PWA check's collector (RB-080-08): one expression the component runs through `evaluate` in
// the plugin's isolated world. It only reads: the manifest links, theme-color, apple-touch-icon,
// whether the context is secure, and the service worker registration of the page (scope, script,
// state) and whether one controls it. No page script; nothing is registered, updated or removed.
// SPDX-License-Identifier: GPL-3.0-or-later
(async () => {
  const within = (promise) =>
    Promise.race([promise, new Promise((resolve) => setTimeout(() => resolve(undefined), 3000))]);
  const href = (selector) => {
    const element = document.querySelector(selector);
    return element && typeof element.href === 'string' ? element.href : null;
  };
  const sw = { supported: false, controlled: false, registration: null, error: null };
  try {
    sw.supported = 'serviceWorker' in navigator;
    if (sw.supported) {
      sw.controlled = !!navigator.serviceWorker.controller;
      const registration = await within(navigator.serviceWorker.getRegistration());
      if (registration) {
        const worker = registration.active || registration.waiting || registration.installing;
        sw.registration = {
          scope: registration.scope,
          script: worker ? worker.scriptURL : null,
          state: worker ? worker.state : null,
          active: !!registration.active,
        };
      }
    }
  } catch (error) {
    sw.error = String((error && error.name) || error).slice(0, 100);
  }
  return {
    url: location.href,
    secure: window.isSecureContext === true,
    manifests: Array.from(document.querySelectorAll('link[rel~="manifest" i]'))
      .map((link) => link.href)
      .filter((value) => typeof value === 'string' && value !== '')
      .slice(0, 4),
    themeColor: document.querySelector('meta[name="theme-color" i][content]') !== null,
    appleTouchIcon: href('link[rel~="apple-touch-icon" i]'),
    sw,
  };
})()
