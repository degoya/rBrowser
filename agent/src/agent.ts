// The agent of one frame (RB-020-01): its configuration, the replay marker, the commands of the
// core, and the messages it sends over the binding. Capture is `capture.ts`, resolution
// `resolve.ts`, the page API's bus and page state `page-api/relay.ts` (RB-020-08); the boot in
// the isolated world is `boot.ts`. Navigation sync (RB-020-06): the Browsersync detection of the
// top frame (`browsersync.ts`) and the SPA follow (`spa.ts`). Scroll sync `scroll/`
// (RB-020-04); form and focus sync `forms/` (RB-020-05). The top frame's Web Vitals `vitals/`
// (RB-060-10) and its frameworks, as the main-world probe reports them (`framework.ts`,
// RB-080-24). The page's direction, language and pseudo-localisation `locale/` (RB-092-02).
// Media that does not play in any frame `media.ts` (RB-099-29).

import { watchBrowsersync } from './browsersync';
import { installCapture, type CaptureGroup } from './capture';
import { applyChange, applyFocus, isChangeState } from './forms/apply';
import { watchFrameworks } from './framework';
import { isPageLocale } from './locale/check';
import { watchMedia } from './media';
import { PageLocaleController } from './locale/apply';
import type {
  AgentCommand,
  AgentConfig,
  AgentMessage,
  CommandResult,
  ElementTarget,
  PathPoint,
  RecordConfig,
  ScrollData,
  SyncEvent,
} from './protocol';
import { DEFAULT_CONFIG, DEFAULT_SCROLL_DEBOUNCE_MS, PROTOCOL_VERSION } from './protocol';
import { BusRelay } from './page-api/relay';
import { resolve } from './resolve';
import { applyScroll, pointOf } from './scroll/apply';
import { cssPath } from './selector';
import { followRoute } from './spa';
import { installVitals } from './vitals/collector';
import { isObject } from '../../plugins/web-common/src/is-object';

/** The longest replay marker the core may set, in ms. */
export const MAX_REPLAY_MS = 10_000;
/** The replay marker a `scroll` command sets itself: the scroll events it causes come in the
 *  next frames and are not captured. */
export const SCROLL_MARKER_MS = 300;

/** How an agent is made. */
export interface AgentOptions {
  win: Window;
  /** Sends one message (JSON text) to the core: the binding. */
  send: (payload: string) => void;
  config?: AgentConfig;
  /** Default: `event.isTrusted` (real input, or the engine's CDP input; never a page script). */
  trusted?: (event: Event) => boolean;
}

/** The CSS paths of the iframes from the top document down to `win`; null when a hop is
 *  cross-origin (the core knows the frame from the CDP session then). */
function framePath(win: Window): string[] | null {
  const path: string[] = [];
  try {
    for (let current = win; current !== current.top; current = current.parent) {
      const frame = current.frameElement;
      if (!frame) return null;
      path.unshift(cssPath(frame));
    }
  } catch {
    return null;
  }
  return path;
}


/** Whether `value` is a running recording's `RecordConfig` (RB-090-87). */
function isRecord(value: unknown): value is RecordConfig {
  return (
    isObject(value) &&
    typeof value.menuHovers === 'boolean' &&
    typeof value.hoverDwellMs === 'number' &&
    value.hoverDwellMs >= 0 &&
    (value.recordPointer === 'menu' || value.recordPointer === 'all' || value.recordPointer === 'off')
  );
}

/** Whether `value` is a complete `AgentConfig`. */
export function isConfig(value: unknown): value is AgentConfig {
  if (!isObject(value) || !isObject(value.capture)) return false;
  const capture = value.capture;
  return (
    (value.record === undefined || isRecord(value.record)) &&
    typeof value.active === 'boolean' &&
    typeof value.hoverThrottleMs === 'number' &&
    value.hoverThrottleMs >= 0 &&
    (value.scrollDebounceMs === undefined ||
      (typeof value.scrollDebounceMs === 'number' && value.scrollDebounceMs >= 0)) &&
    (value.scrollMode === undefined || value.scrollMode === 'anchor' || value.scrollMode === 'wheel') &&
    ['click', 'hover', 'scroll', 'keyboard', 'input', 'forms', 'focus'].every(
      (group) => typeof capture[group] === 'boolean',
    )
  );
}

/** Whether `value` reads as an `ElementTarget` (also the core's, RB-090-53). */
export function isTarget(value: unknown): value is ElementTarget {
  return isObject(value) && isObject(value.element) && isObject(value.offset);
}

/** Whether `value` carries a scroll position (`ScrollData`). */
function isScroll(value: unknown): value is ScrollData {
  return (
    isObject(value) &&
    ['x', 'y', 'maxX', 'maxY'].every((key) => Number.isFinite(value[key])) &&
    (value.container === undefined || isTarget(value.container)) &&
    (value.anchor === undefined || isTarget(value.anchor))
  );
}

/** The agent of one frame. */
export class Agent {
  private readonly win: Window;
  private readonly send: (payload: string) => void;
  private readonly trusted: (event: Event) => boolean;
  private config: AgentConfig;
  private seq = 0;
  private replayUntil = 0;
  private readonly top: boolean;
  private readonly frame: string[] | null;
  private uninstall: (() => void) | null = null;
  /** Follows a new `scrollMode` with the wheel listener (RB-090-33). */
  private refreshCapture: (() => void) | null = null;
  private readonly relay: BusRelay;
  private unwatch: (() => void) | null = null;
  private unvitals: (() => void) | null = null;
  private unframeworks: (() => void) | null = null;
  /** Media that does not play, in every frame (RB-099-29). */
  private unmedia: (() => void) | null = null;
  /** The page's direction, language and pseudo-localisation (RB-092-02). */
  private readonly locale: PageLocaleController;

  constructor(options: AgentOptions) {
    this.win = options.win;
    this.send = options.send;
    this.config = options.config ?? DEFAULT_CONFIG;
    this.trusted = options.trusted ?? ((event) => event.isTrusted);
    this.top = this.win === this.win.top;
    this.frame = this.top ? null : framePath(this.win);
    this.locale = new PageLocaleController(this.win);
    this.relay = new BusRelay(this.win, this.top, (bus) =>
      this.post({ kind: 'bus', v: PROTOCOL_VERSION, ...this.origin(), time: Date.now(), ...bus }),
    );
  }

  /** Starts capturing and says ready. */
  start(): void {
    if (this.uninstall) return;
    const capture = installCapture(this.win, {
      enabled: (group) => this.enabled(group),
      trusted: this.trusted,
      hoverThrottleMs: () => this.config.hoverThrottleMs,
      scrollDebounceMs: () => this.config.scrollDebounceMs ?? DEFAULT_SCROLL_DEBOUNCE_MS,
      wheel: () => this.config.scrollMode === 'wheel',
      record: () => this.config.record ?? null,
      // The way only from the top frame: its points are the viewport's (RB-090-87).
      emit: (event, path) => this.emit(event, this.top ? path : undefined),
    });
    const uninstallRelay = this.relay.start();
    this.refreshCapture = capture.refresh;
    this.uninstall = () => {
      capture.stop();
      uninstallRelay();
    };
    this.post({ kind: 'ready', v: PROTOCOL_VERSION, ...this.origin(), url: this.win.location.href });
    this.unmedia = watchMedia(this.win, (failure) =>
      this.post({ kind: 'media', v: PROTOCOL_VERSION, ...this.origin(), ...failure }),
    );
    if (this.top) {
      this.unwatch = watchBrowsersync(this.win, () =>
        this.post({ kind: 'detected', v: PROTOCOL_VERSION, ...this.origin(), tool: 'browsersync' }),
      );
      this.unvitals = installVitals({
        win: this.win,
        emit: (data) => this.post({ kind: 'vitals', v: PROTOCOL_VERSION, ...this.origin(), ...data }),
      });
      this.unframeworks = watchFrameworks(this.win, (report) =>
        this.post({ kind: 'framework', v: PROTOCOL_VERSION, ...this.origin(), ...report }),
      );
    }
  }

  /** Stops capturing. */
  stop(): void {
    this.uninstall?.();
    this.uninstall = null;
    this.refreshCapture = null;
    this.unwatch?.();
    this.unwatch = null;
    this.unvitals?.();
    this.unvitals = null;
    this.unframeworks?.();
    this.unframeworks = null;
    this.unmedia?.();
    this.unmedia = null;
    this.locale.stop();
  }

  /** Runs a command of the core; never throws. */
  command(input: unknown): CommandResult {
    if (!isObject(input) || typeof input.cmd !== 'string') {
      return { status: 'error', message: 'not a command' };
    }
    const command = input as AgentCommand;
    switch (command.cmd) {
      case 'configure':
        if (!isConfig(command.config)) return { status: 'error', message: 'invalid config' };
        this.config = command.config;
        this.refreshCapture?.();
        return { status: 'done' };
      case 'replay': {
        const ms = Number(command.ms);
        if (!Number.isFinite(ms) || ms < 0) return { status: 'error', message: 'invalid ms' };
        this.replayUntil = ms === 0 ? 0 : Date.now() + Math.min(ms, MAX_REPLAY_MS);
        return { status: 'done' };
      }
      case 'resolve':
        if (!isTarget(command.target)) return { status: 'error', message: 'invalid target' };
        try {
          return resolve(command.target, this.win.document);
        } catch (error) {
          return { status: 'error', message: String(error) };
        }
      case 'scroll':
        if (!isScroll(command)) return { status: 'error', message: 'invalid scroll' };
        this.replayUntil = Math.max(this.replayUntil, Date.now() + SCROLL_MARKER_MS);
        try {
          return applyScroll(command, this.win.document);
        } catch (error) {
          return { status: 'error', message: String(error) };
        }
      case 'point':
        if (!isTarget(command.target)) return { status: 'error', message: 'invalid target' };
        try {
          return pointOf(command.target, this.win.document);
        } catch (error) {
          return { status: 'error', message: String(error) };
        }
      case 'apply':
        if (!isTarget(command.target) || !isChangeState(input)) {
          return { status: 'error', message: 'invalid state' };
        }
        try {
          return applyChange(command, this.win.document);
        } catch {
          // Never the error's text: it might quote a value.
          return { status: 'error', message: 'apply failed' };
        }
      case 'focus':
        if (!isTarget(command.target)) return { status: 'error', message: 'invalid target' };
        try {
          return applyFocus(command.target, this.win.document);
        } catch {
          return { status: 'error', message: 'focus failed' };
        }
      case 'ping':
        return { status: 'done' };
      case 'page':
        return this.relay.configure(command.page);
      case 'bus':
        return this.relay.deliver(input);
      case 'navigate':
        return followRoute(this.win, command.url);
      case 'locale':
        if (!isPageLocale(command.locale)) return { status: 'error', message: 'invalid locale' };
        try {
          this.locale.apply(command.locale);
          return { status: 'done' };
        } catch {
          return { status: 'error', message: 'locale failed' };
        }
      default:
        return { status: 'error', message: 'unknown command' };
    }
  }

  private enabled(group: CaptureGroup): boolean {
    return this.config.active && Date.now() >= this.replayUntil && this.config.capture[group];
  }

  private origin(): { top: boolean; frame?: string[] } {
    return this.frame ? { top: this.top, frame: this.frame } : { top: this.top };
  }

  private emit(event: SyncEvent, path?: PathPoint[]): void {
    this.seq += 1;
    this.post({
      kind: 'event',
      v: PROTOCOL_VERSION,
      ...this.origin(),
      seq: this.seq,
      time: Date.now(),
      event,
      ...(path && path.length > 0 ? { path } : {}),
    });
  }

  private post(message: AgentMessage): void {
    try {
      this.send(JSON.stringify(message));
    } catch {
      // The binding is gone (the frame is being torn down): nothing to report to.
    }
  }
}
