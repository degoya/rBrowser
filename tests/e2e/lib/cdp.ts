// CDP through rBrowser's token proxy, as Playwright's connectOverCDP would go (RB-099-02). The
// endpoint rules are examples/playwright/endpoint.ts's (the `cdp_endpoint` answer, its `wsl`
// route inside WSL) and the client is scripts/lib/perf-measure.mjs's (Node's own fetch and
// WebSocket) - shared, not copied. Nothing here prints a header or a URL with the token.

import { endpointFromAnswer } from '../../../examples/playwright/endpoint.ts';
import type { Endpoint } from '../../../examples/playwright/endpoint.ts';
import { browserSocket, connect, evaluate } from '../../../scripts/lib/perf-measure.mjs';

export type { Endpoint };

export interface TargetInfo {
  targetId: string;
  type: string;
  url: string;
}

export interface Cdp {
  call(method: string, params?: object, sessionId?: string): Promise<Record<string, unknown>>;
  close(): void;
}

/** The URLs no outside client may see (crates/rb-cdp/src/target_filter.rs, K10). */
const HIDDEN = [/^rbrowser:/i, /^devtools:/i, /^about:blank#rb-/i];

/** The targets that should have been hidden and were not. */
export function leakedTargets(infos: readonly TargetInfo[]): TargetInfo[] {
  return infos.filter((info) => HIDDEN.some((pattern) => pattern.test(info.url)));
}

export function pageTargets(infos: readonly TargetInfo[]): TargetInfo[] {
  return infos.filter((info) => info.type === 'page');
}

/** The endpoint of a `cdp_endpoint` answer; `inWsl` takes its route from WSL. */
export function endpointOf(answer: unknown, inWsl: boolean): Endpoint {
  return endpointFromAnswer(answer, inWsl);
}

/** The HTTP status of `/json/version` asked without a token (K10: 401 or 403). */
export async function statusWithout(url: string): Promise<number> {
  const base = new URL(url);
  // The path form carries the token in its first segment: ask the bare origin.
  const answer = await fetch(new URL('/json/version', base.origin), { redirect: 'manual' });
  await answer.arrayBuffer();
  return answer.status;
}

/** Whether a WebSocket to `url` with `headers` opens (closed again at once). */
export async function socketOpens(url: string, headers: Record<string, string>): Promise<boolean> {
  try {
    const cdp = (await connect(url, headers)) as Cdp;
    cdp.close();
    return true;
  } catch {
    return false;
  }
}

/** A flat-session CDP client on the browser behind the proxy. */
export async function openCdp(endpoint: Endpoint): Promise<Cdp> {
  return (await connect(await browserSocket(endpoint), endpoint.headers)) as Cdp;
}

export async function targets(cdp: Cdp): Promise<TargetInfo[]> {
  const answer = await cdp.call('Target.getTargets');
  return (answer.targetInfos ?? []) as TargetInfo[];
}

/** Attaches to every page target, in the proxy's order; answers the session ids. */
export async function attachPages(cdp: Cdp): Promise<{ target: TargetInfo; session: string }[]> {
  const sessions = [];
  for (const target of pageTargets(await targets(cdp))) {
    const answer = await cdp.call('Target.attachToTarget', { targetId: target.targetId, flatten: true });
    sessions.push({ target, session: String(answer.sessionId) });
  }
  return sessions;
}

/** Runs `expression` in the page of `session` and answers its value. */
export async function evaluateIn<T = unknown>(cdp: Cdp, session: string, expression: string): Promise<T> {
  return (await evaluate(cdp, session, expression)) as T;
}

/** A real left click at `x`, `y` of the page of `session` (press, release). */
export async function clickAt(cdp: Cdp, session: string, x: number, y: number): Promise<void> {
  for (const type of ['mousePressed', 'mouseReleased']) {
    await cdp.call('Input.dispatchMouseEvent', { type, x, y, button: 'left', clickCount: 1 }, session);
  }
}

/** The centre of the first element `selector` names in the page of `session`, or null. */
export async function centreOf(cdp: Cdp, session: string, selector: string): Promise<{ x: number; y: number } | null> {
  const expression = `(() => {
    const element = document.querySelector(${JSON.stringify(selector)});
    if (!element) return null;
    element.scrollIntoView({ block: 'center' });
    const box = element.getBoundingClientRect();
    return { x: box.left + box.width / 2, y: box.top + box.height / 2 };
  })()`;
  return evaluateIn(cdp, session, expression);
}
