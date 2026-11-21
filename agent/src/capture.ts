// Capture (RB-020-01): the agent listens on `window` in the capture phase, so the page can
// neither stop an event before the agent sees it nor tell that it is watched (passive listeners,
// nothing changed in the DOM). Hover is reported only when the element under the pointer
// changes, throttled; wheel is coalesced per 16 ms and listened for only in wheel mode
// (RB-090-33), scroll per animation frame with an anchor (`scroll/capture.ts`, RB-020-04). Whether anything is captured at all (active view, replay
// marker, `sync.*` switches) is the host's decision, asked per event. A mouse press inside a
// hover-only submenu reports the hover on its menu entry first (`menu-trigger.ts`, RB-090-76).
// While a recording runs, a rest on a menu's entry is a hover step and the pointer's way goes with
// the step it led to (`record-hover.ts`, RB-090-87). A click on the label of a checkbox or radio
// names the control (`control`); the engine's own click on the control is not sent (RB-090-93).

import type { AgentConfig, MouseButton, PathPoint, RecordConfig, SyncEvent } from './protocol';
import { DEFAULT_SCROLL_DEBOUNCE_MS, MOD_ALT, MOD_CTRL, MOD_META, MOD_SHIFT } from './protocol';
import { frameOf, ScrollCapture } from './scroll/capture';
import { actionable, centreTarget, cssPath, makeTarget } from './selector';
import { changeState, editableTracker, isSensitive, opensPicker } from './forms/capture';
import { menuTrigger } from './menu-trigger';
import { HoverRecording } from './record-hover';

/** A switch of `AgentConfig.capture`. */
export type CaptureGroup = keyof AgentConfig['capture'];

/** What capture needs from the agent. */
export interface CaptureHost {
  /** Whether events of `group` are captured right now. */
  enabled(group: CaptureGroup): boolean;
  /** Whether `event` came from the user (or the engine's input), not from a page script. */
  trusted(event: Event): boolean;
  hoverThrottleMs(): number;
  /** `sync.scrollDebounceMs` (default `DEFAULT_SCROLL_DEBOUNCE_MS`). */
  scrollDebounceMs?(): number;
  /** Whether the wheel is captured (`sync.scrollMode: wheel`, RB-090-33); absent: always. Read
   *  at the start and on every `refresh`. */
  wheel?(): boolean;
  /** A running recording's wishes for this view (RB-090-87); absent or null: none runs. */
  record?(): RecordConfig | null;
  /** `path`: the pointer's way to the event, while a recording keeps it (RB-090-87). */
  emit(event: SyncEvent, path?: PathPoint[]): void;
}

/** Wheel events are coalesced for this long (about a frame). */
export const COALESCE_MS = 16;

const BUTTONS: MouseButton[] = ['left', 'middle', 'right', 'back', 'forward'];

function modifiers(event: MouseEvent | KeyboardEvent): number {
  return (
    (event.altKey ? MOD_ALT : 0) |
    (event.ctrlKey ? MOD_CTRL : 0) |
    (event.metaKey ? MOD_META : 0) |
    (event.shiftKey ? MOD_SHIFT : 0)
  );
}

function buttonOf(button: number): MouseButton {
  return BUTTONS[button] ?? 'none';
}

/** The element an event really happened on, inside open shadow roots; null inside a plugin's
 *  own UI (its host element `rbrowser-<id>`, RB-060-12), which is never synced. The composed
 *  path is read once per event (RB-080-27). */
function deepTarget(event: Event): Element | null {
  const path = event.composedPath();
  const first = path[0];
  const element = first instanceof Element ? first : event.target instanceof Element ? event.target : null;
  return element && throughPluginUi(path) ? null : element;
}

/** Whether the event passed through a plugin's host element (`rbrowser-*`). */
function isPluginUi(event: Event): boolean {
  return throughPluginUi(event.composedPath());
}

function throughPluginUi(path: EventTarget[]): boolean {
  return path.some((node) => node instanceof Element && node.localName.startsWith('rbrowser-'));
}

/** The focused element, inside open shadow roots; null for the body. */
function deepActive(doc: Document): Element | null {
  let active = doc.activeElement;
  while (active?.shadowRoot?.activeElement) active = active.shadowRoot.activeElement;
  if (!active || active === doc.body || active === doc.documentElement) return null;
  return active;
}

function isField(element: Element): element is HTMLInputElement | HTMLTextAreaElement {
  return element.localName === 'input' || element.localName === 'textarea';
}

/** The checkbox or radio a click on `subject` - a label - goes on to; null for anything else. */
function toggleOfLabel(subject: Element): HTMLInputElement | null {
  if (subject.localName !== 'label') return null;
  const control = (subject as HTMLLabelElement).control;
  if (control?.localName !== 'input') return null;
  const type = (control as HTMLInputElement).type;
  return type === 'checkbox' || type === 'radio' ? (control as HTMLInputElement) : null;
}

interface PendingWheel {
  element: Element;
  x: number;
  y: number;
  deltaX: number;
  deltaY: number;
  deltaMode: number;
  modifiers: number;
}

/** The installed listeners. */
export interface Capture {
  /** Adds or removes the wheel listener as `CaptureHost.wheel` says now. */
  refresh(): void;
  /** Removes every listener. */
  stop(): void;
}

/** Installs the listeners on `win`; `stop` removes them again. */
export function installCapture(win: Window, host: CaptureHost): Capture {
  const doc = win.document;
  const timers = new Set<ReturnType<typeof setTimeout>>();
  const later = (ms: number, run: () => void) => {
    const timer = setTimeout(() => {
      timers.delete(timer);
      run();
    }, ms);
    timers.add(timer);
  };

  const editable = editableTracker();
  const recording = new HoverRecording({
    record: () => host.record?.() ?? null,
    hovering: () => host.enabled('hover'),
    emit: (event, path) => host.emit(event, path),
    now: () => Date.now(),
  });
  /** The raw element of the last pointermove: the same one again walks no ancestors. */
  let lastRaw: Element | null = null;
  let lastHover: Element | null = null;
  let hoverAt = Number.NEGATIVE_INFINITY;
  let hoverPending: { element: Element; x: number; y: number } | null = null;
  let hoverTimer = false;
  const flushHover = () => {
    hoverTimer = false;
    const pending = hoverPending;
    hoverPending = null;
    if (!pending || !host.enabled('hover')) return;
    hoverAt = Date.now();
    host.emit({ type: 'hover', target: makeTarget(pending.element, pending.x, pending.y) });
  };

  /** The control of the label just clicked: the engine clicks it next, in the same task. */
  let labelControl: Element | null = null;

  let wheel: PendingWheel | null = null;
  const flushWheel = () => {
    const pending = wheel;
    wheel = null;
    if (!pending || !host.enabled('scroll')) return;
    const { element, x, y, ...rest } = pending;
    host.emit({ type: 'wheel', ...rest, target: makeTarget(element, x, y) });
  };

  const scrolls = new ScrollCapture({
    enabled: () => host.enabled('scroll'),
    emit: (event) => host.emit(event),
    debounceMs: () => host.scrollDebounceMs?.() ?? DEFAULT_SCROLL_DEBOUNCE_MS,
    frame: frameOf(win),
  });

  // Only in wheel mode: anchor mode replays the position, so the wheel is not even listened
  // for (RB-090-33).
  const onWheel = (event: Event): void => {
    const turn = event as WheelEvent;
    const element = deepTarget(turn);
    if (!element || !host.enabled('scroll')) return;
    if (wheel && wheel.element === element && wheel.deltaMode === turn.deltaMode) {
      wheel.deltaX += turn.deltaX;
      wheel.deltaY += turn.deltaY;
      return;
    }
    flushWheel();
    wheel = {
      element,
      x: turn.clientX,
      y: turn.clientY,
      deltaX: turn.deltaX,
      deltaY: turn.deltaY,
      deltaMode: turn.deltaMode,
      modifiers: modifiers(turn),
    };
    later(COALESCE_MS, flushWheel);
  };

  const handlers: Record<string, (event: Event) => void> = {
    pointerdown: (event) => {
      menuHover(event as PointerEvent);
      pointer(event as PointerEvent, 'down');
    },
    pointerup: (event) => pointer(event as PointerEvent, 'up'),
    click: (event) => mouse(event as MouseEvent, 'click'),
    dblclick: (event) => mouse(event as MouseEvent, 'dblclick'),
    contextmenu: (event) => mouse(event as MouseEvent, 'contextmenu'),
    pointermove: (event) => {
      const move = event as PointerEvent;
      if (move.pointerType === 'touch' || !host.enabled('hover')) return;
      recording.moved(move.clientX, move.clientY);
      const element = deepTarget(move);
      if (!element || element === lastRaw) return;
      lastRaw = element;
      const subject = actionable(element);
      if (subject === lastHover) return;
      lastHover = subject;
      recording.entered(subject);
      hoverPending = { element: subject, x: move.clientX, y: move.clientY };
      const wait = hoverAt + host.hoverThrottleMs() - Date.now();
      if (wait <= 0) flushHover();
      else if (!hoverTimer) {
        hoverTimer = true;
        later(wait, flushHover);
      }
    },
    // The pointer leaves the document, the window loses the focus or the tab is hidden: a rest
    // still waiting is none, and the next entry starts anew (RB-090-87).
    pointerout: (event) => {
      if ((event as PointerEvent).relatedTarget === null) leave();
    },
    // The window's own blur: an element's blur targets a node.
    blur: (event) => {
      if (typeof (event.target as { nodeType?: unknown } | null)?.nodeType !== 'number') leave();
    },
    visibilitychange: () => leave(),
    scroll: (event) => {
      const target = event.target;
      scrolls.onScroll(target instanceof Element ? target : doc);
    },
    keydown: (event) => key(event as KeyboardEvent, 'keydown'),
    keyup: (event) => key(event as KeyboardEvent, 'keyup'),
    input: (event) => {
      const element = deepTarget(event);
      if (!element || !host.enabled('input')) return;
      const input = event as InputEvent;
      const captured: Extract<SyncEvent, { type: 'input' }> = {
        type: 'input',
        inputType: input.inputType ?? '',
        target: centreTarget(element),
      };
      if (typeof input.data === 'string') captured.data = input.data;
      if (isField(element) && (element as HTMLInputElement).type !== 'file') {
        captured.value = element.value;
      }
      if (isSensitive(element)) captured.sensitive = true;
      host.emit(captured);
    },
    change: (event) => {
      const element = deepTarget(event);
      if (!element || !host.enabled('forms')) return;
      host.emit({ type: 'change', ...changeState(element), target: centreTarget(element) });
    },
    // A contenteditable reports its text as a change when it loses the focus (RB-020-05).
    focusin: (event) => {
      const element = deepTarget(event);
      if (element) editable.focusin(element);
    },
    focusout: (event) => {
      const element = deepTarget(event);
      const edited = element ? editable.focusout(element) : null;
      if (!edited || !host.enabled('forms')) return;
      host.emit({ type: 'change', value: edited.value, target: centreTarget(edited.host) });
    },
    submit: (event) => {
      const form = deepTarget(event);
      if (!(form instanceof HTMLFormElement) || !host.enabled('input')) return;
      host.emit({
        type: 'submit',
        method: (form.getAttribute('method') ?? 'get').toLowerCase(),
        action: form.action,
        target: centreTarget(form),
      });
    },
    focus: (event) => {
      const element = deepTarget(event);
      if (!element || !host.enabled('focus')) return;
      host.emit({ type: 'focus', target: centreTarget(element) });
    },
  };

  // A mouse press inside a hover-only submenu reports the hover on its menu entry first, so a
  // recording (and a follower) opens the submenu before the click (RB-090-76, `menu-trigger.ts`).
  function leave(): void {
    recording.left();
    lastRaw = null;
    lastHover = null;
  }

  // While a recording runs, the press ends a rest still waiting and takes the click's way; the
  // trigger's hover is not reported again when the last rest was on its menu (RB-090-87).
  function menuHover(event: PointerEvent): void {
    if ((event.pointerType || 'mouse') !== 'mouse') return;
    const element = deepTarget(event);
    if (!element || event.button !== 0 || !host.enabled('hover') || !host.enabled('click')) {
      recording.pressed(element ?? doc.documentElement, null, event.clientX, event.clientY);
      return;
    }
    const trigger = menuTrigger(element);
    const report = recording.pressed(element, trigger, event.clientX, event.clientY);
    if (trigger && report) host.emit({ type: 'hover', target: centreTarget(trigger), menu: true });
  }

  function pointer(event: PointerEvent, phase: 'down' | 'up'): void {
    const element = deepTarget(event);
    if (!element || !host.enabled('click')) return;
    host.emit({
      type: 'pointer',
      phase,
      button: buttonOf(event.button),
      buttons: event.buttons,
      pointerType: event.pointerType || 'mouse',
      modifiers: modifiers(event),
      target: makeTarget(element, event.clientX, event.clientY),
    });
  }

  function mouse(event: MouseEvent, type: 'click' | 'dblclick' | 'contextmenu'): void {
    const element = deepTarget(event);
    if (type === 'click') {
      // The engine's click on a label's checkbox or radio (RB-090-93): the label's click does it
      // again in every view, and the control's `change` belongs to that click.
      const engines = labelControl !== null && labelControl === element;
      labelControl = null;
      if (engines) return;
    }
    if (!element || !host.enabled('click')) return;
    // A native picker would open in every view; the control's value follows with `change`.
    if (type !== 'contextmenu' && opensPicker(element)) return;
    const captured: Extract<SyncEvent, { type: 'click' | 'dblclick' | 'contextmenu' }> = {
      type,
      button: buttonOf(event.button),
      detail: event.detail,
      modifiers: modifiers(event),
      target: makeTarget(element, event.clientX, event.clientY),
    };
    const control = type === 'click' ? toggleOfLabel(actionable(element)) : null;
    if (control) {
      captured.control = cssPath(control);
      labelControl = control;
      // Its click comes in this task or not at all (a page that cancels the label's click).
      later(0, () => {
        if (labelControl === control) labelControl = null;
      });
    }
    host.emit(captured, type === 'click' ? recording.clickPath() : undefined);
  }

  function key(event: KeyboardEvent, type: 'keydown' | 'keyup'): void {
    // An IME composition is sent as its result (input), not as its keys.
    if (event.isComposing || event.key === 'Process' || !host.enabled('keyboard')) return;
    if (isPluginUi(event)) return;
    const focused = deepActive(doc);
    const captured: Extract<SyncEvent, { type: 'keydown' | 'keyup' }> = {
      type,
      key: event.key,
      code: event.code,
      keyCode: event.keyCode || 0,
      location: event.location,
      repeat: event.repeat,
      modifiers: modifiers(event),
    };
    if (focused) {
      captured.target = centreTarget(focused);
      if (isSensitive(focused)) captured.sensitive = true;
    }
    host.emit(captured);
  }

  const trustedOnly = (handle: (event: Event) => void) => (event: Event) => {
    if (host.trusted(event)) handle(event);
  };
  const listeners = Object.entries(handlers).map(([type, handle]) => {
    const listener = trustedOnly(handle);
    win.addEventListener(type, listener, { capture: true, passive: true });
    return [type, listener] as const;
  });

  let wheelListener: ((event: Event) => void) | null = null;
  const refresh = (): void => {
    const wanted = host.wheel?.() ?? true;
    if (wanted && !wheelListener) {
      wheelListener = trustedOnly(onWheel);
      win.addEventListener('wheel', wheelListener, { capture: true, passive: true });
    } else if (!wanted && wheelListener) {
      win.removeEventListener('wheel', wheelListener, { capture: true });
      wheelListener = null;
      wheel = null;
    }
  };
  refresh();

  return {
    refresh,
    stop: () => {
      for (const [type, listener] of listeners) {
        win.removeEventListener(type, listener, { capture: true });
      }
      if (wheelListener) win.removeEventListener('wheel', wheelListener, { capture: true });
      wheelListener = null;
      for (const timer of timers) clearTimeout(timer);
      timers.clear();
      scrolls.dispose();
      recording.stop();
    },
  };
}
