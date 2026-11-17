// The prologue of a plugin's page scripts (RB-060-12, docs/plugins.md "Page scripts and
// panels"). It runs first in the plugin's isolated world and defines `rbPlugin`; the host puts
// its configuration in front as `const C = {...};` and each script after it as
// `run("<run_at>", function (rbPlugin) { ... });`. The page never sees this world.
const g = globalThis;
const live = g[C.receiver];
if (live && !live.stopped) return;
const call = g[C.binding];
if (typeof call !== "function") return;
try {
  delete g[C.binding];
} catch (_) {}
const pending = new Map();
const listeners = new Set();
const stops = new Set();
let next = 1;
let stopped = false;
const size = (text) =>
  text.length * 3 <= C.max ? text.length : new TextEncoder().encode(text).length;
const encode = (message) => {
  const text = JSON.stringify(message === undefined ? null : message);
  if (size(text) > C.max) throw new Error("plugins.error.too_large");
  return text;
};
// RB-080-66: a line for rBrowser's log and console, never the page's console (`l:` - the
// host's PageLog): at most 16 texts of 1000 characters, an error by its message; an error line
// carries the first error's stack (4000 characters), so a page script's throw keeps it.
const LEVELS = { error: "error", warn: "warning", warning: "warning", info: "info", log: "log", debug: "debug" };
// An error of any realm (a DOMException included).
const isError = (value) =>
  value instanceof Error ||
  (value && typeof value === "object" && typeof value.message === "string" && typeof value.name === "string");
// Well-formed (a lone surrogate half becomes U+FFFD) and cut on a character, never through a
// surrogate pair: the host's JSON refuses a lone half and would lose the line.
const cut = (value, max) => {
  let out = typeof value.toWellFormed === "function" ? value.toWellFormed() : value;
  if (out.length > max) {
    out = out.slice(0, max);
    const last = out.charCodeAt(out.length - 1);
    if (last >= 0xd800 && last <= 0xdbff) out = out.slice(0, -1);
  }
  return out;
};
const text = (value) => {
  if (isError(value)) return value.message || String(value);
  if (typeof value === "string") return value;
  try {
    const json = JSON.stringify(value);
    return json === undefined ? String(value) : json;
  } catch (_) {
    return String(value);
  }
};
const log = (level, values) => {
  if (stopped) return;
  try {
    const line = { level: LEVELS[level] || "log", values: values.slice(0, 16).map((value) => cut(text(value), 1000)) };
    const failed = line.level === "error" && values.find(isError);
    if (failed && typeof failed.stack === "string" && failed.stack) line.stack = cut(failed.stack, 4000);
    call("l:" + encode(line));
  } catch (_) {}
};
// A page script's own throw: rBrowser's failure, with its stack.
const crash = (error) => log("error", ["[" + C.id + "]", error]);
const settle = (id, ok, value) => {
  const entry = pending.get(id);
  if (!entry) return;
  pending.delete(id);
  clearTimeout(entry.timer);
  if (ok) entry.resolve(value);
  else entry.reject(new Error(String(value)));
};
// RB-090-51: every overlay a page script draws is a child of one `<rbrowser-layers>` at the end
// of `<html>` (docs/plugins.md "Overlays"), found by its name on every call - the scripts of
// other plugins run in other worlds, which share the DOM but no JavaScript - made by the first
// that needs it, gone with its last layer. `display: contents`: no box, no stacking context, no
// containing block, so a layer is placed as if it were a child of `<html>`. The same logic for
// bundled scripts is plugins/web-common/src/layers.ts; layers-spec.ts tests both. RB-090-64: each
// layer is `aria-hidden`, never the container - Chrome blocks it on an ancestor of the focused
// element and warns in the page's console - except one that takes focus: `add(host,
// { interactive: true, role, label })` gives it a role (default `dialog`) and its name instead.
const LAYERS = "rbrowser-layers";
const mark = (host, options) => {
  if (options && options.interactive) {
    host.removeAttribute("aria-hidden");
    host.setAttribute("role", String(options.role || "dialog"));
    if (options.label) host.setAttribute("aria-label", String(options.label));
  } else {
    if (host.getAttribute("aria-hidden") !== "true") host.setAttribute("aria-hidden", "true");
    host.removeAttribute("role");
    host.removeAttribute("aria-label");
  }
};
// An older copy hid the container itself: its unmarked layers (no aria-hidden, no role) are
// hidden one by one before the container's attribute goes.
const unhide = (box) => {
  if (!box.hasAttribute("aria-hidden")) return;
  for (const child of box.children) {
    if (!child.hasAttribute("aria-hidden") && !child.hasAttribute("role")) child.setAttribute("aria-hidden", "true");
  }
  box.removeAttribute("aria-hidden");
};
const layersOf = (doc) => {
  const root = doc.documentElement;
  if (root) for (const child of root.children) if (child.localName === LAYERS) return child;
  return null;
};
const layers = Object.freeze({
  add(host, options) {
    const doc = host.ownerDocument;
    const root = doc.documentElement;
    if (stopped || !root) return false;
    let box = layersOf(doc);
    if (!box) {
      box = doc.createElement(LAYERS);
      // The CSSOM, which a page's CSP (`style-src`) cannot refuse.
      box.style.setProperty("all", "initial", "important");
      box.style.setProperty("display", "contents", "important");
      root.append(box);
    }
    unhide(box);
    mark(host, options);
    if (host.getAttribute("data-rbrowser-plugin") !== C.id) host.setAttribute("data-rbrowser-plugin", C.id);
    if (host.parentNode !== box) box.append(host);
    return true;
  },
  remove(host) {
    const parent = host.parentElement;
    host.remove();
    if (parent && parent.localName === LAYERS && !parent.firstElementChild) parent.remove();
  },
});
const api = Object.freeze({
  id: C.id,
  layers,
  view: C.view,
  maxMessageBytes: C.max,
  send(message) {
    if (stopped) return Promise.reject(new Error("plugins.error.stopped"));
    let text;
    try {
      text = encode(message);
    } catch (error) {
      return Promise.reject(error);
    }
    const id = next++;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => settle(id, false, "plugins.error.no_answer"), C.timeout);
      pending.set(id, { resolve, reject, timer });
      call("s" + id + ":" + text);
    });
  },
  emit(message) {
    if (!stopped) call("e:" + encode(message));
  },
  log(level, ...values) {
    log(String(level), values);
  },
  on(listener) {
    listeners.add(listener);
    return () => listeners.delete(listener);
  },
  onStop(listener) {
    stops.add(listener);
    return () => stops.delete(listener);
  },
});
const receive = (message) => {
  if (!message || typeof message !== "object") return;
  if (typeof message.reply === "number") {
    settle(message.reply, message.ok === true, message.ok === true ? message.value : message.error);
  } else if ("post" in message) {
    for (const listener of [...listeners]) {
      try {
        listener(message.post);
      } catch (error) {
        crash(error);
      }
    }
  } else if (message.stop === true && !stopped) {
    stopped = true;
    receive.stopped = true;
    for (const id of [...pending.keys()]) settle(id, false, "plugins.error.stopped");
    for (const listener of [...stops]) {
      try {
        listener();
      } catch (error) {
        crash(error);
      }
    }
    listeners.clear();
    stops.clear();
    delete g.rbPlugin;
  }
};
receive.stopped = false;
// The world's own console (the page never sees it) goes to rBrowser as well, so no plugin's
// script writes into the page's console.
const own = g.console;
if (own) {
  for (const method of ["error", "warn", "info", "log", "debug"]) {
    try {
      own[method] = (...values) => log(method, values);
    } catch (_) {}
  }
}
Object.defineProperty(g, C.receiver, { value: receive, configurable: true });
Object.defineProperty(g, "rbPlugin", { value: api, configurable: true });
const run = (at, body) => {
  const go = () => {
    if (stopped) return;
    try {
      body(api);
    } catch (error) {
      crash(error);
    }
  };
  const state = document.readyState;
  if (at === "document-start" || state === "complete" || (at === "document-end" && state !== "loading")) {
    go();
  } else if (at === "document-end") {
    document.addEventListener("DOMContentLoaded", go, { once: true });
  } else {
    g.addEventListener("load", go, { once: true });
  }
};
