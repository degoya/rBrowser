// Hovers and the pointer's way for a running recording (RB-090-87). The core sends
// `AgentConfig.record` to the view it records from; without it nothing here runs.
//
// - A rest: when the pointer stays on one actionable element for `hoverDwellMs` and that element
//   is a menu's entry (`isMenuEntry`: a menu entry whose hover shows a submenu, or one inside a
//   submenu or an ARIA menu, any level), its hover is reported as a step (`menu: true`), with or
//   without a click after it. Passing over an entry quicker reports nothing. An entry whose
//   submenu has not opened yet (a menu with a hover delay) is looked at again while the pointer
//   stays, up to `RECHECK_LIMIT_MS`.
// - At a mouse press the waiting rest is dropped (the click is the step), and the capture's menu
//   hover of RB-090-76 is not reported again when the last rest was on that menu already.
// - The way (`pointer-path.ts`): with `recordPointer: all` every rest and click gets the way that
//   led to it; with `menu` a rest, and a click into a menu or while one is open (a rest was
//   reported since the last press); with `off` none.

import { hasSubmenu, isMenuEntry, menuTrigger } from './menu-trigger';
import { PointerPath } from './pointer-path';
import type { PathPoint, RecordConfig, SyncEvent } from './protocol';
import { actionable, makeTarget } from './selector';

/** How often an entry whose submenu is not shown yet is looked at again, in ms. */
export const RECHECK_MS = 100;
/** How long after the rest an entry is looked at again. */
export const RECHECK_LIMIT_MS = 1_000;

/** What the hover recording needs from the capture. */
export interface RecordHoverHost {
  /** The recording's wishes; null while none runs in this view. */
  record(): RecordConfig | null;
  /** Whether hovers are captured right now. */
  hovering(): boolean;
  emit(event: SyncEvent, path?: PathPoint[]): void;
  /** Now, in ms. */
  now(): number;
}

/** The rests and the way of one frame. */
export class HoverRecording {
  private readonly path = new PointerPath();
  private timer: ReturnType<typeof setTimeout> | null = null;
  private x = 0;
  private y = 0;
  private movedAt = 0;
  /** The element the last rest was reported on, since the last press. */
  private lastRest: Element | null = null;
  /** The way of the click that follows the press. */
  private pressPath: PathPoint[] | undefined;

  constructor(private readonly host: RecordHoverHost) {}

  /** A mouse move to (`x`, `y`). */
  moved(x: number, y: number): void {
    this.x = x;
    this.y = y;
    this.movedAt = this.host.now();
    const record = this.host.record();
    if (record && record.recordPointer !== 'off') this.path.add(x, y, this.movedAt);
  }

  /** The pointer is on another actionable element now. */
  entered(element: Element): void {
    this.cancel();
    const record = this.host.record();
    if (!record?.menuHovers) return;
    const dwell = Math.max(0, record.hoverDwellMs);
    this.schedule(dwell, () => this.rest(element, dwell));
  }

  /** A mouse press on `element`, whose menu trigger is `trigger`: answers whether the capture
   *  reports the trigger's hover (RB-090-76) - not when the last rest was on that menu. */
  pressed(element: Element, trigger: Element | null, x: number, y: number): boolean {
    this.cancel();
    const record = this.host.record();
    const rest = this.lastRest;
    this.lastRest = null;
    this.pressPath = undefined;
    if (!record) {
      this.path.clear();
      return true;
    }
    const intoMenu = rest !== null || trigger !== null || isMenuEntry(element);
    const keep = record.recordPointer === 'all' || (record.recordPointer === 'menu' && intoMenu);
    if (keep) {
      const way = this.path.take(x, y, this.host.now());
      if (way.length > 0) this.pressPath = way;
    } else {
      this.path.clear();
    }
    const opened = rest !== null && trigger !== null && (rest === trigger || menuTrigger(rest) === trigger);
    return !opened;
  }

  /** The way for the click after the press, once. */
  clickPath(): PathPoint[] | undefined {
    const way = this.pressPath;
    this.pressPath = undefined;
    return way;
  }

  /** The pointer left the page (to the browser's toolbar), the window lost the focus or the tab
   *  was hidden: the rest still waiting is none. */
  left(): void {
    this.cancel();
  }

  /** Stops the waiting rest. */
  stop(): void {
    this.cancel();
    this.path.clear();
  }

  private rest(element: Element, waited: number): void {
    this.timer = null;
    const record = this.host.record();
    if (!record?.menuHovers || !this.host.hovering() || !element.isConnected) return;
    if (!isMenuEntry(element)) {
      // A menu with a hover delay: its submenu may still open while the pointer stays.
      if (hasSubmenu(element) && waited + RECHECK_MS <= RECHECK_LIMIT_MS + record.hoverDwellMs) {
        this.schedule(RECHECK_MS, () => this.rest(element, waited + RECHECK_MS));
      }
      return;
    }
    const subject = actionable(element);
    const way =
      record.recordPointer === 'off' ? [] : this.path.take(this.x, this.y, this.movedAt);
    this.lastRest = subject;
    this.host.emit(
      { type: 'hover', target: makeTarget(subject, this.x, this.y), menu: true },
      way.length > 0 ? way : undefined,
    );
  }

  private schedule(ms: number, run: () => void): void {
    this.timer = setTimeout(run, ms);
  }

  private cancel(): void {
    if (this.timer !== null) clearTimeout(this.timer);
    this.timer = null;
  }
}
