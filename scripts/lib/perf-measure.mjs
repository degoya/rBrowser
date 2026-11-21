#!/usr/bin/env node
// One measuring run of scripts/perf.sh (RB-020-09): over rBrowser's CDP proxy it scrolls one
// view with real wheel input for a number of seconds and writes, as JSON, the frame rate of
// that view (a requestAnimationFrame counter in the page) and each view's main-thread time
// (`Performance.getMetrics` TaskDuration) - the second is what shows a throttled view working
// less. Memory is sampled by perf.sh from the Windows process list.
//
//   node scripts/lib/perf-measure.mjs --seconds 10 --view 0 --out run-1.json
//
// The endpoint comes from the environment, as in examples/playwright/endpoint.ts:
// RBROWSER_CDP_URL (+ RBROWSER_CDP_TOKEN), or RBROWSER_CDP_ENDPOINT naming a file with the
// answer of the MCP tool `cdp_endpoint` (inside WSL its `wsl` route). No message here carries
// the token. Node's own fetch and WebSocket, no packages. The CDP helpers are shared with
// scripts/lib/perf-k1.mjs (the click latency, K1).

import { readFileSync, writeFileSync } from 'node:fs';

const HELP =
  'set RBROWSER_CDP_URL (and RBROWSER_CDP_TOKEN), or RBROWSER_CDP_ENDPOINT to a file with the answer of the MCP tool cdp_endpoint';
const CALL_TIMEOUT_MS = 15000;
const WHEEL_INTERVAL_MS = 50;
const WHEEL_DELTA = 120;
// Down for this long, then up: the fixture page is tall enough at every breakpoint.
const DIRECTION_MS = 2000;

function args(argv) {
  const options = { seconds: 10, view: 0, out: '' };
  for (let i = 0; i < argv.length; i += 1) {
    const name = argv[i];
    const value = argv[i + 1];
    if (name === '--seconds') options.seconds = Number(value);
    else if (name === '--view') options.view = Number(value);
    else if (name === '--out') options.out = value ?? '';
    else throw new Error(`unknown argument ${name}`);
    i += 1;
  }
  if (!Number.isFinite(options.seconds) || options.seconds < 1 || options.seconds > 600) {
    throw new Error('--seconds is 1 to 600');
  }
  if (!Number.isInteger(options.view) || options.view < 0) {
    throw new Error('--view is a view index from 0');
  }
  return options;
}

function text(value) {
  return typeof value === 'string' && value.length > 0 ? value : undefined;
}

/** The proxy URL and headers from the environment (endpoint.ts's rules). */
export function endpoint(env) {
  const url = text(env.RBROWSER_CDP_URL);
  if (url !== undefined) {
    const token = text(env.RBROWSER_CDP_TOKEN);
    return { url, headers: token === undefined ? {} : { Authorization: `Bearer ${token}` } };
  }
  const file = text(env.RBROWSER_CDP_ENDPOINT);
  if (file === undefined) throw new Error(`no rBrowser CDP endpoint: ${HELP}`);
  let answer;
  try {
    answer = JSON.parse(readFileSync(file, 'utf8'));
  } catch {
    throw new Error(`RBROWSER_CDP_ENDPOINT is no readable JSON file: ${file}`);
  }
  if (answer === null || typeof answer !== 'object' || text(answer.url) === undefined) {
    throw new Error('the cdp_endpoint answer has no URL: switch the proxy on in Settings > Automation > CDP proxy');
  }
  const headers = {};
  for (const [name, value] of Object.entries(answer.headers ?? {})) {
    if (typeof value === 'string') headers[name] = value;
  }
  let target = answer.url;
  if (text(env.WSL_DISTRO_NAME) !== undefined && answer.wsl && typeof answer.wsl === 'object') {
    if (text(answer.wsl.error) !== undefined) throw new Error(`no route from WSL: ${answer.wsl.error}`);
    target = text(answer.wsl.url) ?? target;
  }
  return { url: target, headers };
}

/** The browser's WebSocket URL behind the proxy URL. */
export async function browserSocket(proxy) {
  if (proxy.url.startsWith('ws://') || proxy.url.startsWith('wss://')) return proxy.url;
  const base = proxy.url.endsWith('/') ? proxy.url : `${proxy.url}/`;
  const answer = await fetch(new URL('json/version', base), { headers: proxy.headers });
  if (!answer.ok) throw new Error(`the CDP proxy answered ${answer.status} to /json/version`);
  const version = await answer.json();
  const socket = text(version.webSocketDebuggerUrl);
  if (socket === undefined) throw new Error('the CDP proxy named no webSocketDebuggerUrl');
  return socket;
}

/** A flat-session CDP client over one WebSocket. */
export async function connect(url, headers) {
  const socket = new WebSocket(url, { headers });
  await new Promise((resolve, reject) => {
    socket.addEventListener('open', resolve, { once: true });
    socket.addEventListener('error', () => reject(new Error('the CDP WebSocket did not open')), {
      once: true,
    });
  });
  let next = 0;
  const pending = new Map();
  socket.addEventListener('message', (event) => {
    const message = JSON.parse(String(event.data));
    const waiting = pending.get(message.id);
    if (waiting === undefined) return;
    pending.delete(message.id);
    if (message.error) waiting.reject(new Error(`${waiting.method}: ${message.error.message}`));
    else waiting.resolve(message.result ?? {});
  });
  const call = (method, params = {}, sessionId = undefined) =>
    new Promise((resolve, reject) => {
      next += 1;
      const id = next;
      const timer = setTimeout(() => {
        pending.delete(id);
        reject(new Error(`${method}: no answer in ${CALL_TIMEOUT_MS} ms`));
      }, CALL_TIMEOUT_MS);
      pending.set(id, {
        method,
        resolve: (value) => {
          clearTimeout(timer);
          resolve(value);
        },
        reject: (error) => {
          clearTimeout(timer);
          reject(error);
        },
      });
      socket.send(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) }));
    });
  return { call, close: () => socket.close() };
}

export async function evaluate(cdp, session, expression) {
  const answer = await cdp.call(
    'Runtime.evaluate',
    { expression, returnByValue: true, awaitPromise: true },
    session,
  );
  return answer.result?.value;
}

async function taskSeconds(cdp, session) {
  const answer = await cdp.call('Performance.getMetrics', {}, session);
  const metric = (answer.metrics ?? []).find((entry) => entry.name === 'TaskDuration');
  return typeof metric?.value === 'number' ? metric.value : 0;
}

const START_FRAMES = `(() => {
  const frames = [];
  window.__rbPerf = { frames, stop: false };
  const tick = (time) => { frames.push(time); if (!window.__rbPerf.stop) requestAnimationFrame(tick); };
  requestAnimationFrame(tick);
  return true;
})()`;
const STOP_FRAMES = `(() => { window.__rbPerf.stop = true; return window.__rbPerf.frames; })()`;
const VIEW_FACTS = `({ url: location.href, width: innerWidth, dpr: devicePixelRatio })`;

/** Frames per second and the 95th percentile frame time of the `frames` timestamps (ms). */
export function frameStats(frames) {
  if (frames.length < 2) return { fps: 0, p95FrameMs: 0, frames: frames.length };
  const intervals = [];
  for (let i = 1; i < frames.length; i += 1) intervals.push(frames[i] - frames[i - 1]);
  const total = frames[frames.length - 1] - frames[0];
  intervals.sort((a, b) => a - b);
  const p95 = intervals[Math.min(intervals.length - 1, Math.floor(intervals.length * 0.95))];
  const round = (value) => Math.round(value * 10) / 10;
  return { fps: round(((frames.length - 1) * 1000) / total), p95FrameMs: round(p95), frames: frames.length };
}

async function measure(options) {
  const proxy = endpoint(process.env);
  const cdp = await connect(await browserSocket(proxy), proxy.headers);
  try {
    const { targetInfos } = await cdp.call('Target.getTargets');
    const pages = (targetInfos ?? []).filter((info) => info.type === 'page');
    if (options.view >= pages.length) {
      throw new Error(`--view ${options.view}: the proxy shows ${pages.length} view(s)`);
    }
    const views = [];
    for (const page of pages) {
      const { sessionId } = await cdp.call('Target.attachToTarget', { targetId: page.targetId, flatten: true });
      await cdp.call('Performance.enable', {}, sessionId);
      const facts = (await evaluate(cdp, sessionId, VIEW_FACTS)) ?? {};
      views.push({ targetId: page.targetId, session: sessionId, ...facts });
    }
    const measured = views[options.view];
    await evaluate(cdp, measured.session, 'window.scrollTo(0, 0)');
    const before = await Promise.all(views.map((view) => taskSeconds(cdp, view.session)));
    await evaluate(cdp, measured.session, START_FRAMES);
    const started = Date.now();
    while (Date.now() - started < options.seconds * 1000) {
      const down = Math.floor((Date.now() - started) / DIRECTION_MS) % 2 === 0;
      await cdp.call(
        'Input.dispatchMouseEvent',
        { type: 'mouseWheel', x: 100, y: 200, deltaX: 0, deltaY: down ? WHEEL_DELTA : -WHEEL_DELTA },
        measured.session,
      );
      await new Promise((resolve) => setTimeout(resolve, WHEEL_INTERVAL_MS));
    }
    const frames = (await evaluate(cdp, measured.session, STOP_FRAMES)) ?? [];
    const elapsed = (Date.now() - started) / 1000;
    const after = await Promise.all(views.map((view) => taskSeconds(cdp, view.session)));
    return {
      seconds: Math.round(elapsed * 10) / 10,
      measuredView: options.view,
      ...frameStats(frames),
      views: views.map((view, index) => ({
        index,
        url: view.url ?? '',
        width: view.width ?? 0,
        dpr: view.dpr ?? 0,
        cpuSeconds: Math.round((after[index] - before[index]) * 1000) / 1000,
        cpuShare: Math.round(((after[index] - before[index]) / elapsed) * 1000) / 1000,
      })),
    };
  } finally {
    cdp.close();
  }
}

if (import.meta.url === `file://${process.argv[1]}`) {
  try {
    const options = args(process.argv.slice(2));
    const run = await measure(options);
    const json = `${JSON.stringify(run, null, 2)}\n`;
    if (options.out) writeFileSync(options.out, json);
    else process.stdout.write(json);
  } catch (error) {
    process.stderr.write(`perf-measure: ${error instanceof Error ? error.message : String(error)}\n`);
    process.exit(1);
  }
}
