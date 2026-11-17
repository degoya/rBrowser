// The bridge between plugin panels and the core (RB-060-12, docs/plugins.md "Page scripts and
// panels"). Each panel runs in an `<iframe sandbox="allow-scripts">` of the panels window: an
// opaque origin with no `cefQuery` and no network. It reaches its own plugin - and only its own -
// through `postMessage` to the panels window, which this bridge answers:
//
//   panel -> host: { type: 'call', id, command | tool, args? }  answered with `result`
//                  { type: 'settings' }                          answered with `settings`
//                  { type: 'dirty', dirty }   whether it holds unsaved work (RB-080-59): the
//                                             window keeps its frame alive while it does
//                  { type: 'keep' }           keep this frame alive, hidden, while another panel
//                                             is shown (RB-110-16): a panel slow to fill, so a
//                                             switch back shows it at once
//                  { target: 'host', action: 'open_link', url, id? }   a link the person clicked
//                                             (RB-080-92): http(s) only, only within a user
//                                             gesture, one per plugin per LINK_INTERVAL_MS; opened
//                                             through the core's `app.open_link`, answered with
//                                             `result` ({ opened }) when it carries an id
//   host -> panel: { type: 'result', id, ok: true, value }
//                  { type: 'result', id, ok: false, error, params? }  the code and its parameters
//                                             (`reason`, ...): the panel says it as a sentence
//                  { type: 'event', message, view? }   from its page scripts or its component
//                  { type: 'settings', strings, settings }  on request and when settings change
//                                             or the text size does: `settings` carries the
//                                             shell's text size as `ui.textScale` (RB-090-39)
//
// A message counts only from a frame the bridge attached (`event.source`), with the opaque
// origin `"null"`; anything else, any other type, a malformed or oversized message is dropped.
// Oversized is over `plugins.panelMessageMaxBytes` (RB-090-44, 4 MiB by default), which the core
// sends with every panel's settings; a call over it is answered `plugins.error.too_large` with
// the limit and the setting's name.

import { isOpenableUrl } from '../links/open';
import { parseTextScale } from '../theme/textScale';
import type { PanelEvent, PanelMessage, PanelRequest } from './protocol';

/** The key of the shell's text size in a panel's settings, in percent (RB-090-39): a panel's
 * frame is a document of its own, so it draws its root font size from it (panel-ui). */
export const TEXT_SCALE_KEY = 'ui.textScale';
/** The setting that holds the largest message, as JSON text in bytes, in either direction. */
export const MESSAGE_LIMIT_KEY = 'plugins.panelMessageMaxBytes';
/** Its default until the core sent it (`PluginSettings` of rb-config): 4 MiB. */
export const DEFAULT_MESSAGE_BYTES = 4 * 1024 * 1024;
/** Its smallest and largest value: the page scripts' 64 KiB, and 16 MiB. */
export const MESSAGE_BYTES_RANGE: readonly [number, number] = [64 * 1024, 16 * 1024 * 1024];
/** The reason of a message over the limit; its text names the setting. */
export const LIMIT_REASON = 'panel_message_limit';
/** The most calls of one plugin's panels waiting at once; more are answered `busy`. */
export const MAX_PENDING = 16;
/** How long the panels window waits for a call (RB-080-90): a plugin call may run for its
 * manifest's time (Fonts: 60 s, three times that on the wall clock) and first wait for a running
 * one of the same plugin (a check run); the shell's 10 s gave it up while the core still worked
 * on it. The core bounds every call, so this is a backstop only. */
export const PANEL_CALL_TIMEOUT_MS = 10 * 60 * 1000;
/** How long the same given-up call stays out of the log after it was written. */
export const LOG_EVERY_MS = 60_000;
/** The most characters of a failure's text a panel gets. */
const MAX_FAILURE_CHARS = 1000;
/** The origin a sandboxed frame without `allow-same-origin` sends from. */
export const OPAQUE_ORIGIN = 'null';
/** The least time between two links one plugin's panels open. */
export const LINK_INTERVAL_MS = 1000;

/** What a panel may send. */
export type FromPanel =
  | { type: 'call'; id: number; command?: string; tool?: string; args?: unknown }
  | { type: 'settings' }
  | { type: 'dirty'; dirty: boolean }
  | { type: 'keep' }
  | { type: 'openLink'; url: string; id?: number };

/** How the window opens a panel's link, and whether the person just acted. */
export interface PanelLinks {
  /** Opens `url` (a checked web address) through the core; `false` when it did not. */
  open(url: string): Promise<boolean>;
  /** Whether the window has a user gesture now: a click in a panel's frame counts for the
   * window too (user activation reaches every ancestor frame). */
  gesture(): boolean;
  /** The clock, ms. */
  now(): number;
}

/** No way to open a link: the window gave none. */
const NO_LINKS: PanelLinks = {
  open: async () => false,
  gesture: () =>
    (globalThis.navigator as (Navigator & { userActivation?: { isActive: boolean } }) | undefined)?.userActivation
      ?.isActive ?? false,
  now: () => Date.now(),
};

/** What a panel gets. */
export type ToPanel =
  | { type: 'result'; id: number; ok: true; value: unknown }
  | { type: 'result'; id: number; ok: false; error: string; params?: Record<string, string> }
  | { type: 'event'; message: unknown; view?: number }
  | { type: 'settings'; strings: Record<string, string>; settings: Record<string, unknown> };

/** The core's side: a `pluginPanel` request and its answer's message; a call is sent with how
 * long to wait for it. */
export type Ask = (request: PanelRequest, options?: { timeoutMs: number }) => Promise<PanelMessage>;

/** The window's side: an attached panel holds unsaved work (`dirty`) or no longer does. */
export type OnDirty = (target: PanelTarget, dirty: boolean) => void;

/** The window's side: an attached panel asks to keep its frame alive while hidden. */
export type OnKeep = (target: PanelTarget) => void;

/** A frame's window as the bridge posts to it. */
export interface PanelTarget {
  postMessage(message: unknown, targetOrigin: string): void;
}

/** A `message` event as the bridge reads it. */
export interface IncomingMessage {
  source: unknown;
  origin: string;
  data: unknown;
}

const NAME = /^[a-z][a-z0-9_]{0,39}$/;

/** The message limit `settings` name, held to its range; `undefined` when they name none. */
export function messageLimitOf(settings: Record<string, unknown>): number | undefined {
  const value = settings[MESSAGE_LIMIT_KEY];
  if (typeof value !== 'number' || !Number.isSafeInteger(value)) {
    return undefined;
  }
  const [least, most] = MESSAGE_BYTES_RANGE;
  return Math.min(Math.max(value, least), most);
}

/** The size of `value` as JSON text in bytes; `Infinity` for what has no JSON. */
export function jsonBytes(value: unknown): number {
  try {
    const text = JSON.stringify(value ?? null);
    return new TextEncoder().encode(text).length;
  } catch {
    return Infinity;
  }
}

/** `data` as a panel's message, or `null` when it is none of the fixed list. */
export function parseFromPanel(data: unknown): FromPanel | null {
  if (typeof data !== 'object' || data === null || Array.isArray(data)) {
    return null;
  }
  const message = data as Record<string, unknown>;
  if (message.target === 'host' && message.action === 'open_link') {
    if (typeof message.url !== 'string') {
      return null;
    }
    const link: FromPanel = { type: 'openLink', url: message.url };
    return Number.isSafeInteger(message.id) ? { ...link, id: message.id as number } : link;
  }
  if (message.type === 'settings') {
    return { type: 'settings' };
  }
  if (message.type === 'dirty') {
    return typeof message.dirty === 'boolean' ? { type: 'dirty', dirty: message.dirty } : null;
  }
  if (message.type === 'keep') {
    return { type: 'keep' };
  }
  if (message.type !== 'call' || !Number.isSafeInteger(message.id)) {
    return null;
  }
  const command = typeof message.command === 'string' ? message.command : undefined;
  const tool = typeof message.tool === 'string' ? message.tool : undefined;
  const name = command ?? tool;
  if ((command === undefined) === (tool === undefined) || name === undefined || !NAME.test(name)) {
    return null;
  }
  const call: FromPanel = { type: 'call', id: message.id as number, args: message.args };
  return command !== undefined ? { ...call, command } : { ...call, tool };
}

const CODE = /^[a-z][a-z0-9_-]*(\.[a-z0-9_-]+)+$/;

/** The code of a failed call: the core's message starts with it (`plugins.error.*`, the plugin's
 * own key); anything else - the transport's timeout - is `plugins.error.host_failed`. */
export function codeOf(error: unknown): string {
  const text = error instanceof Error ? error.message : String(error);
  const first = text.split(' ')[0] ?? '';
  return CODE.test(first) ? first : 'plugins.error.host_failed';
}

const PARAM = / ([A-Za-z_][A-Za-z0-9_]*)=/g;
const NO_ANSWER = /^no answer to \S+ in \d+ ms$/;

/** A failed call as the panel gets it: the code and its parameters. The core writes a failure
 * as `<code> <key>=<value> ...` (a value may hold spaces: an exception's message); the
 * transport's give-up is `plugins.error.host_failed` with `reason=no_answer` (RB-080-90). */
export function failureOf(error: unknown): { code: string; params: Record<string, string> } {
  const text = (error instanceof Error ? error.message : String(error)).slice(0, MAX_FAILURE_CHARS);
  const code = codeOf(error);
  if (code !== (text.split(' ')[0] ?? '')) {
    return { code, params: NO_ANSWER.test(text) ? { reason: 'no_answer' } : {} };
  }
  const params: Record<string, string> = {};
  const marks = [...text.matchAll(PARAM)];
  marks.forEach((mark, index) => {
    const name = mark[1] ?? '';
    const start = (mark.index ?? 0) + mark[0].length;
    params[name] = text.slice(start, marks[index + 1]?.index ?? text.length);
  });
  return { code, params };
}

export class PanelBridge {
  private readonly frames = new Map<unknown, { plugin: string; target: PanelTarget }>();
  private readonly pending = new Map<string, number>();
  /** When a given-up call of a plugin was last written to the log, by `plugin/call`. */
  private readonly logged = new Map<string, number>();
  /** When each plugin's panels opened a link last. */
  private readonly linked = new Map<string, number>();
  private readonly links: PanelLinks;
  private limit = DEFAULT_MESSAGE_BYTES;
  private textScale = parseTextScale(undefined);

  constructor(
    private readonly ask: Ask,
    private readonly onDirty: OnDirty = () => undefined,
    links: Partial<PanelLinks> = {},
    private readonly onKeep: OnKeep = () => undefined,
  ) {
    this.links = { ...NO_LINKS, ...links };
  }

  /** The largest message now, in bytes: the setting the core sent last, else its default. */
  get messageLimit(): number {
    return this.limit;
  }

  /** Talks to `target` as a panel of `plugin` from now on; the function detaches it. */
  attach(plugin: string, target: PanelTarget): () => void {
    this.frames.set(target, { plugin, target });
    return () => {
      this.frames.delete(target);
    };
  }

  /** A `message` event of the panels window; `true` when it came from an attached panel. */
  async receive(event: IncomingMessage): Promise<boolean> {
    const frame = this.frames.get(event.source);
    if (frame === undefined || event.origin !== OPAQUE_ORIGIN) {
      return false;
    }
    if (jsonBytes(event.data) > this.limit) {
      const id = (event.data as { id?: unknown } | null)?.id;
      if (Number.isSafeInteger(id)) {
        frame.target.postMessage(this.tooLarge(id as number), '*');
      }
      return true;
    }
    const message = parseFromPanel(event.data);
    if (message === null) {
      return true;
    }
    if (message.type === 'settings') {
      await this.sendSettings(frame.plugin, frame.target);
    } else if (message.type === 'dirty') {
      this.onDirty(frame.target, message.dirty);
    } else if (message.type === 'keep') {
      this.onKeep(frame.target);
    } else if (message.type === 'openLink') {
      await this.openLink(frame.plugin, frame.target, message);
    } else {
      await this.call(frame.plugin, frame.target, message);
    }
    return true;
  }

  /** Hands `event` to every attached panel of its plugin. */
  deliver(event: PanelEvent): void {
    const message: ToPanel =
      event.view === undefined
        ? { type: 'event', message: event.message }
        : { type: 'event', message: event.message, view: event.view };
    for (const frame of this.frames.values()) {
      if (frame.plugin === event.plugin) {
        this.post(frame.target, message);
      }
    }
  }

  /** Takes the shell's text size in percent; a new one sends every panel its settings again. */
  async setTextScale(percent: number): Promise<void> {
    const next = parseTextScale(percent);
    if (next === this.textScale) {
      return;
    }
    this.textScale = next;
    await this.refreshSettings();
  }

  /** Sends every attached panel its plugin's settings again (the settings changed). */
  async refreshSettings(): Promise<void> {
    await Promise.all([...this.frames.values()].map((frame) => this.sendSettings(frame.plugin, frame.target)));
  }

  private async call(plugin: string, target: PanelTarget, message: Extract<FromPanel, { type: 'call' }>) {
    const waiting = this.pending.get(plugin) ?? 0;
    if (waiting >= MAX_PENDING) {
      this.post(target, { type: 'result', id: message.id, ok: false, error: 'plugins.error.busy' });
      return;
    }
    this.pending.set(plugin, waiting + 1);
    try {
      const request: PanelRequest = { op: 'call', plugin, args: message.args ?? {} };
      if (message.command !== undefined) {
        request.command = message.command;
      } else {
        request.tool = message.tool;
      }
      const answer = await this.ask(request, { timeoutMs: PANEL_CALL_TIMEOUT_MS });
      const value = 'value' in answer ? answer.value : null;
      this.post(target, { type: 'result', id: message.id, ok: true, value });
    } catch (error) {
      const { code, params } = failureOf(error);
      if (params.reason === 'no_answer') {
        this.logGivenUp(plugin, message.command ?? message.tool ?? '', `${code} reason=no_answer`);
      }
      const failed: Extract<ToPanel, { ok: false }> = { type: 'result', id: message.id, ok: false, error: code };
      this.post(target, Object.keys(params).length > 0 ? { ...failed, params } : failed);
    } finally {
      this.pending.set(plugin, (this.pending.get(plugin) ?? 1) - 1);
    }
  }

  /** RB-080-90: a call the panels window gave up never reached the core's log (the core
   * answered it later, to nobody); it goes to the log here, once a minute per plugin and call. */
  private logGivenUp(plugin: string, call: string, failure: string): void {
    const key = `${plugin}/${call}`;
    const now = this.links.now();
    const last = this.logged.get(key);
    if (last !== undefined && now - last < LOG_EVERY_MS) {
      return;
    }
    this.logged.set(key, now);
    console.warn(`plugin panel of ${plugin}: ${call} failed: ${failure}`);
  }

  /** A panel's link: a web address, within a gesture, not too soon after the last one. */
  private async openLink(plugin: string, target: PanelTarget, message: Extract<FromPanel, { type: 'openLink' }>) {
    const answer = (result: { ok: true; value: unknown } | { ok: false; error: string }) => {
      if (message.id !== undefined) {
        this.post(target, { type: 'result', id: message.id, ...result });
      }
    };
    if (!isOpenableUrl(message.url)) {
      answer({ ok: false, error: /^https?:/i.test(message.url) ? 'links.error.invalid' : 'links.error.scheme' });
      return;
    }
    if (!this.links.gesture()) {
      answer({ ok: false, error: 'links.error.gesture' });
      return;
    }
    const now = this.links.now();
    const last = this.linked.get(plugin);
    if (last !== undefined && now - last < LINK_INTERVAL_MS) {
      answer({ ok: false, error: 'links.error.busy' });
      return;
    }
    this.linked.set(plugin, now);
    try {
      answer({ ok: true, value: { opened: await this.links.open(message.url) } });
    } catch (error) {
      answer({ ok: false, error: codeOf(error) });
    }
  }

  private async sendSettings(plugin: string, target: PanelTarget): Promise<void> {
    try {
      const answer = await this.ask({ op: 'settings', plugin });
      if ('strings' in answer) {
        this.limit = messageLimitOf(answer.settings) ?? this.limit;
        const settings = { ...answer.settings, [TEXT_SCALE_KEY]: this.textScale };
        this.post(target, { type: 'settings', strings: answer.strings, settings });
      }
    } catch (error) {
      // Nothing to tell the panel; it asks again (RB-080-59). The code only - never the text.
      console.warn(`plugin panel of ${plugin}: no texts and settings (${codeOf(error)})`);
    }
  }

  /** The answer to call `id` over the limit: the code, the limit and the setting's name. */
  private tooLarge(id: number): ToPanel {
    const params = { limit: String(this.limit), reason: LIMIT_REASON, setting: MESSAGE_LIMIT_KEY };
    return { type: 'result', id, ok: false, error: 'plugins.error.too_large', params };
  }

  private post(target: PanelTarget, message: ToPanel): void {
    if (message.type !== 'settings' && jsonBytes(message) > this.limit) {
      if (message.type === 'result') {
        target.postMessage(this.tooLarge(message.id), '*');
      }
      return;
    }
    // The frame's origin is opaque: `*` is the only target origin that reaches it, and the
    // message goes to that one window only.
    target.postMessage(message, '*');
  }
}
