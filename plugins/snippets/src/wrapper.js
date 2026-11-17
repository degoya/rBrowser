// The wrapper of every snippet (RB-070-04, src/script.rs): __LIMIT__, __MAX_TEXT__ and
// __MAX_VALUE__ are filled in, the snippet's code follows as the body of the async function this
// expression is called with, then `\n})`. Tested in plugins/snippets/web (wrapper.test.ts).
((__rbSnippetRun) => {
  const LIMIT = __LIMIT__;
  const MAX_TEXT = __MAX_TEXT__;
  const MAX_VALUE = __MAX_VALUE__;
  const MAX_DEPTH = 6;
  const MAX_ITEMS = 100;
  const describeNode = (node) => {
    if (node.nodeType === 1) {
      const id = node.id ? '#' + node.id : '';
      const cls = typeof node.className === 'string' && node.className.trim()
        ? '.' + node.className.trim().split(/\s+/).join('.') : '';
      return '<' + node.nodeName.toLowerCase() + id + cls + '>';
    }
    if (node.nodeType === 3) return '#text ' + JSON.stringify(String(node.nodeValue).slice(0, 80));
    return node.nodeName.toLowerCase();
  };
  const kind = (value) => {
    if (value === null) return 'null';
    if (Array.isArray(value)) return 'array';
    if (typeof Node !== 'undefined' && value instanceof Node) return 'node';
    if (value instanceof Error) return 'error';
    return typeof value;
  };
  const plain = (value, depth, stack) => {
    const type = typeof value;
    if (value === null || type === 'boolean' || type === 'string') return value;
    if (type === 'number') return Number.isFinite(value) ? value : String(value);
    if (type === 'undefined') return null;
    if (type === 'bigint') return String(value) + 'n';
    if (type === 'symbol') return String(value);
    if (type === 'function') return 'function ' + (value.name || '(anonymous)') + '()';
    if (typeof Node !== 'undefined' && value instanceof Node) return describeNode(value);
    if (typeof Window !== 'undefined' && value instanceof Window) return '[Window]';
    if (value instanceof Error) return { name: value.name, message: String(value.message), stack: String(value.stack || '').slice(0, 4000) };
    if (value instanceof Date) return Number.isNaN(value.getTime()) ? 'Invalid Date' : value.toISOString();
    if (value instanceof RegExp) return String(value);
    if (typeof Promise !== 'undefined' && value instanceof Promise) return 'Promise';
    if (stack.includes(value)) return '[Circular]';
    if (depth >= MAX_DEPTH) return Array.isArray(value) ? '[Array(' + value.length + ')]' : '[Object]';
    stack.push(value);
    try {
      if (value instanceof Map) return plain(Array.from(value.entries()), depth, stack);
      if (value instanceof Set) return plain(Array.from(value.values()), depth, stack);
      if (Array.isArray(value) || ArrayBuffer.isView(value)) {
        const items = Array.from(value).slice(0, MAX_ITEMS).map((item) => plain(item, depth + 1, stack));
        if (value.length > MAX_ITEMS) items.push('... ' + (value.length - MAX_ITEMS) + ' more');
        return items;
      }
      const out = {};
      const keys = Object.keys(value);
      for (const key of keys.slice(0, MAX_ITEMS)) {
        let item;
        try { item = value[key]; } catch (error) { item = '[getter threw]'; }
        out[key] = plain(item, depth + 1, stack);
      }
      if (keys.length > MAX_ITEMS) out['...'] = (keys.length - MAX_ITEMS) + ' more';
      return out;
    } finally {
      stack.pop();
    }
  };
  const done = (value) => {
    const tree = plain(value, 0, []);
    const json = JSON.stringify(tree);
    const text = typeof value === 'string' ? JSON.stringify(value) : (value === undefined ? 'undefined' : json);
    const result = { ok: true, type: kind(value), text: String(text).slice(0, MAX_TEXT) };
    if (json !== undefined && json.length <= MAX_VALUE) result.value = tree;
    else result.truncated = true;
    return result;
  };
  const failed = (error, timedOut) => {
    const text = error && error.name ? error.name + ': ' + error.message : String(error);
    const result = { ok: false, type: 'error', text: text.slice(0, MAX_TEXT), timedOut };
    if (error && error.stack) result.stack = String(error.stack).slice(0, 4000);
    return result;
  };
  let timer;
  const expired = {};
  const timeout = new Promise((resolve) => { timer = setTimeout(() => resolve(expired), LIMIT); });
  let running;
  try {
    running = Promise.resolve(__rbSnippetRun());
  } catch (error) {
    running = Promise.reject(error);
  }
  return Promise.race([running, timeout]).then(
    (value) => {
      clearTimeout(timer);
      if (value === expired) return failed(new Error('The snippet did not finish within ' + LIMIT + ' ms.'), true);
      return done(value);
    },
    (error) => {
      clearTimeout(timer);
      return failed(error, false);
    },
  );
})(async () => {
