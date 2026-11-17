// The portal agent's connection (RB-080-05): the page on the device opens a WebSocket to the
// portal it came from (`/__rbrowser/ws`; the device cookie proves it), follows the grid's
// navigation and replays the leader's events, and answers each replay with its status. Beside
// that it names the origins the page failed to load from (RB-090-72, `./misses`), the origin
// only; nothing else: a device drives nothing in rBrowser. A dropped connection is retried a
// few times with a growing pause; a disconnected device gets 403 and stops trying.

import type { SyncEvent } from '../protocol';
import { replayOnDevice, type DeviceStatus } from './replay';

/** The device socket's path on the portal. */
export const SOCKET_PATH = '/__rbrowser/ws';
/** Attempts without a successful open before the agent gives up. */
export const MAX_FAILURES = 5;
/** The first pause before a new attempt, doubled each time up to `MAX_PAUSE_MS`. */
export const FIRST_PAUSE_MS = 1000;
export const MAX_PAUSE_MS = 10_000;

/** A message of the portal. */
export type ToDevice = { kind: 'navigate'; path: string } | { kind: 'event'; event: SyncEvent };

/** A message to the portal. */
export type Report =
  | { kind: 'hello' }
  | { kind: 'replayed'; status: DeviceStatus }
  | { kind: 'missed'; origin: string; via?: string };

/** A path on this origin: `/...`, never `//host` (another origin). */
export function isLocalPath(path: unknown): path is string {
  return typeof path === 'string' && path.startsWith('/') && !path.startsWith('//') && !path.includes('\\');
}

/** Whether `value` is a message of the portal. */
export function isToDevice(value: unknown): value is ToDevice {
  if (typeof value !== 'object' || value === null) return false;
  const message = value as Record<string, unknown>;
  if (message.kind === 'navigate') return isLocalPath(message.path);
  if (message.kind === 'event') {
    const event = message.event as Record<string, unknown> | null;
    return typeof event === 'object' && event !== null && typeof event.type === 'string';
  }
  return false;
}

/** The socket's URL for the page at `location`. */
export function socketUrl(location: Pick<Location, 'protocol' | 'host'>): string {
  return `${location.protocol === 'https:' ? 'wss:' : 'ws:'}//${location.host}${SOCKET_PATH}`;
}

/** What a window needs here. */
export interface PortalWindow {
  location: Pick<Location, 'protocol' | 'host' | 'pathname' | 'search' | 'hash' | 'assign'>;
  document: Document;
  setTimeout(handler: () => void, ms: number): unknown;
}

/** Acts on `message`; the report to send, if any. */
export function handle(message: ToDevice, win: PortalWindow): Report | null {
  if (message.kind === 'navigate') {
    const here = `${win.location.pathname}${win.location.search}${win.location.hash}`;
    if (here !== message.path) win.location.assign(message.path);
    return null;
  }
  return { kind: 'replayed', status: replayOnDevice(message.event, win.document) };
}

/** The part of `WebSocket` used here. */
export interface SocketLike {
  onopen: ((event: Event) => void) | null;
  onmessage: ((event: MessageEvent) => void) | null;
  onclose: ((event: CloseEvent) => void) | null;
  send(data: string): void;
}

export type SocketFactory = (url: string) => SocketLike;

/** The agent's connection with its retries. */
export class PortalClient {
  private failures = 0;
  private pause = FIRST_PAUSE_MS;
  /** The socket while it is open. */
  private current: SocketLike | null = null;
  /** The origins reported so far, with how the page asked; a new socket hears them all again. */
  private readonly missed: Array<[string, string | undefined]> = [];

  constructor(
    private readonly win: PortalWindow,
    private readonly open: SocketFactory,
  ) {}

  /** Connects; reconnects after a drop until `MAX_FAILURES` attempts in a row failed. */
  start(): void {
    let opened = false;
    const socket = this.open(socketUrl(this.win.location));
    socket.onopen = () => {
      opened = true;
      this.failures = 0;
      this.pause = FIRST_PAUSE_MS;
      this.current = socket;
      socket.send(JSON.stringify({ kind: 'hello' } satisfies Report));
      for (const [origin, via] of this.missed) this.sendMissed(socket, origin, via);
    };
    socket.onmessage = (event) => {
      let message: unknown;
      try {
        message = JSON.parse(String(event.data));
      } catch {
        return;
      }
      if (!isToDevice(message)) return;
      const report = handle(message, this.win);
      if (report) socket.send(JSON.stringify(report));
    };
    socket.onclose = () => {
      if (this.current === socket) this.current = null;
      if (!opened) this.failures += 1;
      if (this.failures >= MAX_FAILURES) return;
      const pause = this.pause;
      this.pause = Math.min(this.pause * 2, MAX_PAUSE_MS);
      this.win.setTimeout(() => this.start(), pause);
    };
  }

  /** The page failed to load from `origin`, asked for `via` (each once, capped by `MissedOrigins`). */
  reportMissed(origin: string, via?: string): void {
    this.missed.push([origin, via]);
    if (this.current) this.sendMissed(this.current, origin, via);
  }

  private sendMissed(socket: SocketLike, origin: string, via: string | undefined): void {
    const report: Report = via === undefined ? { kind: 'missed', origin } : { kind: 'missed', origin, via };
    socket.send(JSON.stringify(report));
  }
}
