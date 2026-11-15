// The `webkit.*` methods (RB-040-04): WebKit through playwright-core, headed, one browser
// process and window per view (RB-080-40). rBrowser's `WebkitEngine`
// (crates/rbrowser/src/engines/webkit/) is the only caller.
//
// | Method | Parameters | Answer |
// | --- | --- | --- |
// | `webkit.launch` | `{ agent?: string }` (the sync agent bundle) | `{ version }` (of the last browser started, null before the first) |
// | `webkit.view.create` | `{ view, url?, options, agentConfig? }` | `{ pid, version }` (the view's browser process, null when unknown) |
// | `webkit.view.navigate` | `{ view, url }` | `{}` |
// | `webkit.view.reload`, `.back`, `.forward`, `.raise`, `.close` | `{ view }` | `{}` |
// | `webkit.view.emulate` | `{ view, options }` | `{ recreated }` |
// | `webkit.view.input` | `{ view, events, before?, blockSubmitMs? }` | `{}` |
// | `webkit.view.scroll` | `{ view, x, y, maxX, maxY }` | `{}` |
// | `webkit.view.screenshot` | `{ view, fullPage?, scale? }` (`css` or `device`) | `{ pngBase64 }` |
// | `webkit.view.agent` | `{ view, command }` | the agent's answer |
// | `webkit.view.cookies` | `{ view, cookies }` (RB-040-05, see cookies.ts) | `{ added, refused }` |
// | `webkit.shutdown` | - | `{}` (every view and its browser) |
//
// Notifications: see service.ts.

import type { Router } from '../router.ts';
import { cookies } from './cookies.ts';
import { parseEvents } from './input.ts';
import { bool, finite, integer, object, screenshotScale, text, url, viewId, viewOptions } from './params.ts';
import { WebkitService } from './service.ts';

/** The largest agent bundle accepted (agent/scripts/bundle.mjs keeps it under 50 kB). */
const MAX_AGENT_BYTES = 512 * 1024;
/** The longest submit block (`rb_sync`'s follow window is shorter). */
const MAX_BLOCK_MS = 10_000;
/** The largest scroll position, in CSS px. */
const MAX_SCROLL = 10_000_000;

const done = {};

function params(value: unknown): Record<string, unknown> {
  return value === undefined || value === null ? {} : object(value, 'params');
}

export function registerWebkit(router: Router, service: WebkitService): void {
  router.register('webkit.launch', (raw) => {
    const p = params(raw);
    const agent = p.agent === undefined || p.agent === null ? null : text(p.agent, 'agent', MAX_AGENT_BYTES);
    return service.launch(agent);
  });
  router.register('webkit.view.create', (raw) => {
    const p = params(raw);
    const target = p.url === undefined || p.url === null || p.url === '' ? null : url(p.url);
    return service.create(viewId(p), target, viewOptions(p.options), p.agentConfig ?? null);
  });
  router.register('webkit.view.navigate', async (raw) => {
    const p = params(raw);
    await service.navigate(viewId(p), url(p.url));
    return done;
  });
  const simple: Array<[string, (view: number) => Promise<void>]> = [
    ['webkit.view.reload', (view) => service.reload(view)],
    ['webkit.view.back', (view) => service.back(view)],
    ['webkit.view.forward', (view) => service.forward(view)],
    ['webkit.view.raise', (view) => service.raise(view)],
    ['webkit.view.close', (view) => service.close(view)],
  ];
  for (const [method, run] of simple) {
    router.register(method, async (raw) => {
      await run(viewId(params(raw)));
      return done;
    });
  }
  router.register('webkit.view.emulate', (raw) => {
    const p = params(raw);
    return service.emulate(viewId(p), viewOptions(p.options));
  });
  router.register('webkit.view.input', async (raw) => {
    const p = params(raw);
    const block = p.blockSubmitMs === undefined ? 0 : integer(p.blockSubmitMs, 'blockSubmitMs', 0, MAX_BLOCK_MS);
    await service.input(viewId(p), parseEvents(p.events), p.before ?? null, block);
    return done;
  });
  router.register('webkit.view.scroll', async (raw) => {
    const p = params(raw);
    const at = (key: string) => finite(p[key], key, 0, MAX_SCROLL);
    await service.scroll(viewId(p), { x: at('x'), y: at('y'), maxX: at('maxX'), maxY: at('maxY') });
    return done;
  });
  router.register('webkit.view.screenshot', (raw) => {
    const p = params(raw);
    const fullPage = p.fullPage === undefined ? false : bool(p.fullPage, 'fullPage');
    return service.screenshot(viewId(p), fullPage, p.scale === undefined ? null : screenshotScale(p.scale));
  });
  router.register('webkit.view.agent', (raw) => {
    const p = params(raw);
    return service.agent(viewId(p), object(p.command, 'command'));
  });
  router.register('webkit.view.cookies', (raw) => {
    const p = params(raw);
    return service.addCookies(viewId(p), cookies(p.cookies));
  });
  router.register('webkit.shutdown', async () => {
    await service.shutdown();
    return done;
  });
}
