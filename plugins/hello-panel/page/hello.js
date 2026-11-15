// The page script of hello-panel (RB-060-12): it runs in the plugin's own isolated world of every
// view, where `rbPlugin` is the bridge to the plugin (docs/plugins.md "Page scripts and panels").
// It says hello to the component, tells the panel which page it is on, and shows the greetings
// the component posts as a badge in a shadow root - removed again when the host stops it.
if (window === window.top) {
  const host = document.createElement('div');
  host.setAttribute('data-rbrowser-plugin', rbPlugin.id);
  host.style.cssText = 'position:fixed;right:8px;bottom:8px;z-index:2147483647;pointer-events:none';
  const badge = document.createElement('span');
  badge.style.cssText =
    'font:12px system-ui,sans-serif;padding:4px 8px;border-radius:4px;background:#1f2430;color:#fff;opacity:.9';
  host.attachShadow({ mode: 'closed' }).append(badge);

  const show = (text) => {
    badge.textContent = String(text).slice(0, 200);
    if (!host.isConnected) {
      document.documentElement.append(host);
    }
  };

  rbPlugin.on((message) => {
    if (message && typeof message.show === 'string') {
      show(message.show);
    }
  });
  rbPlugin.onStop(() => host.remove());

  rbPlugin.emit({ loaded: location.href, view: rbPlugin.view });
  rbPlugin
    .send({ url: location.href, title: document.title })
    .then((answer) => show(answer.greeting))
    .catch((error) => console.warn('[hello-panel]', error.message));
}
