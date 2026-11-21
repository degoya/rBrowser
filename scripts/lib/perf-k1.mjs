#!/usr/bin/env node
// The click run of scripts/perf.sh (RB-090-32, K1): over rBrowser's CDP proxy it opens the
// fixture tests/fixtures/site/latency.html in the measured view (navigation sync takes the
// others along), clicks its targets one after the other with real mouse input and reads, per
// view, when the page logged each click (`window.__replayLog`, epoch milliseconds). K1 is the
// time from the leader's click to the same click in every other view: p50, p95 and the slowest
// per view and over all of them, and the clicks a view never got (missed).
//
//   node scripts/lib/perf-k1.mjs --clicks 100 --view 0 --out k1.json
//
// The endpoint comes from the environment as for scripts/lib/perf-measure.mjs, whose CDP
// helpers this uses. No message here carries the token.

import { writeFileSync } from 'node:fs';
import { browserSocket, connect, endpoint, evaluate } from './perf-measure.mjs';

// Between two clicks: a replay and its queue drain, so one click does not wait for the last.
const CLICK_INTERVAL_MS = 250;
// After the last click, for the slowest view's replay.
const SETTLE_MS = 1500;
// How long the views may take to load the fixture.
const LOAD_TIMEOUT_MS = 20000;
const FIXTURE = '/latency.html';
// The fixture has 100 targets, t1 to t100.
const TARGETS = 100;

function args(argv) {
  const options = { clicks: 100, view: 0, out: '' };
  for (let i = 0; i < argv.length; i += 1) {
    const name = argv[i];
    const value = argv[i + 1];
    if (name === '--clicks') options.clicks = Number(value);
    else if (name === '--view') options.view = Number(value);
    else if (name === '--out') options.out = value ?? '';
    else throw new Error(`unknown argument ${name}`);
    i += 1;
  }
  if (!Number.isInteger(options.clicks) || options.clicks < 1 || options.clicks > 1000) {
    throw new Error('--clicks is 1 to 1000');
  }
  if (!Number.isInteger(options.view) || options.view < 0) {
    throw new Error('--view is a view index from 0');
  }
  return options;
}

/** The nearest-rank percentile `p` (0 to 100) of the sorted numbers; 0 for none. */
function percentile(sorted, p) {
  if (sorted.length === 0) return 0;
  const rank = Math.ceil((p / 100) * sorted.length);
  return sorted[Math.min(Math.max(rank, 1), sorted.length) - 1];
}

function stats(deltas, missed) {
  const sorted = [...deltas].sort((a, b) => a - b);
  const round = (value) => Math.round(value * 10) / 10;
  return {
    pairs: sorted.length,
    missed,
    p50Ms: round(percentile(sorted, 50)),
    p95Ms: round(percentile(sorted, 95)),
    maxMs: round(sorted.length === 0 ? 0 : sorted[sorted.length - 1]),
  };
}

/** Each click of a log by its target and how often that target was clicked before it. */
function keyed(log) {
  const seen = new Map();
  const out = new Map();
  for (const entry of log) {
    if (typeof entry?.target !== 'string' || typeof entry.at !== 'number') continue;
    const n = (seen.get(entry.target) ?? 0) + 1;
    seen.set(entry.target, n);
    out.set(`${entry.target}#${n}`, entry.at);
  }
  return out;
}

/**
 * The click latency of the other views' logs `views` against the leader's log `leader`
 * (entries `{ target, at }`; a miss has no target): per view and over all of them.
 */
export function clickLatency(leader, views) {
  const leaderClicks = keyed(leader);
  const all = [];
  let missedAll = 0;
  const perView = views.map((log) => {
    const clicks = keyed(log);
    const deltas = [];
    let missed = 0;
    for (const [key, at] of leaderClicks) {
      const there = clicks.get(key);
      if (there === undefined) missed += 1;
      else deltas.push(there - at);
    }
    all.push(...deltas);
    missedAll += missed;
    return stats(deltas, missed);
  });
  return { views: perView, overall: stats(all, missedAll) };
}

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

const ON_FIXTURE = `location.pathname.endsWith(${JSON.stringify(FIXTURE)}) && Array.isArray(window.__replayLog) && document.readyState === 'complete'`;

const centre = (n) => `(() => {
  const target = document.querySelector('[data-testid="t${n}"]');
  if (!target) return null;
  const box = target.getBoundingClientRect();
  return { x: box.x + box.width / 2, y: box.y + box.height / 2 };
})()`;

async function click(cdp, session, point) {
  const base = { x: point.x, y: point.y, button: 'left', clickCount: 1 };
  await cdp.call('Input.dispatchMouseEvent', { type: 'mouseMoved', x: point.x, y: point.y }, session);
  await cdp.call('Input.dispatchMouseEvent', { type: 'mousePressed', ...base }, session);
  await cdp.call('Input.dispatchMouseEvent', { type: 'mouseReleased', ...base }, session);
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
    const sessions = [];
    for (const page of pages) {
      const { sessionId } = await cdp.call('Target.attachToTarget', { targetId: page.targetId, flatten: true });
      sessions.push(sessionId);
    }
    const leader = sessions[options.view];
    const origin = await evaluate(cdp, leader, 'location.origin');
    await cdp.call('Page.navigate', { url: `${origin}${FIXTURE}` }, leader);
    const started = Date.now();
    let ready = sessions.map(() => false);
    while (Date.now() - started < LOAD_TIMEOUT_MS && !ready.every(Boolean)) {
      await sleep(250);
      ready = await Promise.all(
        sessions.map((session) => evaluate(cdp, session, ON_FIXTURE).catch(() => false)),
      );
    }
    if (!ready[options.view]) throw new Error(`the measured view did not load ${FIXTURE}`);
    for (let i = 0; i < options.clicks; i += 1) {
      const point = await evaluate(cdp, leader, centre((i % TARGETS) + 1));
      if (!point) throw new Error(`${FIXTURE} has no target t${(i % TARGETS) + 1}`);
      await click(cdp, leader, point);
      await sleep(CLICK_INTERVAL_MS);
    }
    await sleep(SETTLE_MS);
    const logs = await Promise.all(
      sessions.map(async (session) => {
        const text = await evaluate(cdp, session, 'JSON.stringify(window.__replayLog ?? [])').catch(() => '[]');
        return JSON.parse(typeof text === 'string' ? text : '[]');
      }),
    );
    const others = logs.filter((_, index) => index !== options.view);
    const result = clickLatency(logs[options.view], others);
    const indexes = sessions.map((_, index) => index).filter((index) => index !== options.view);
    return {
      clicks: options.clicks,
      measuredView: options.view,
      notLoaded: indexes.filter((index) => !ready[index]),
      overall: result.overall,
      views: result.views.map((view, n) => ({ index: indexes[n], ...view })),
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
    process.stderr.write(`perf-k1: ${error instanceof Error ? error.message : String(error)}\n`);
    process.exit(1);
  }
}
