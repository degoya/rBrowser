// Capture (RB-020-01): the agent listens on `window` in the capture phase, so the page can
// neither stop an event before the agent sees it nor tell that it is watched (passive listeners,
// nothing changed in the DOM). Hover is reported only when the element under the pointer
// changes, throttled; wheel is coalesced per 16 ms, scroll per animation frame with an anchor
// (`scroll/capture.ts`, RB-020-04). Whether anything is captured at all (active view, replay
// marker, `sync.*` switches) is the host's decision, asked per event.

import type { AgentConfig, MouseButton, SyncEvent } from './protocol';
import { DEFAULT_SCROLL_DEBOUNCE_MS, MOD_ALT, MOD_CTRL, MOD_META, MOD_SHIFT } from './protocol';
import { frameOf, ScrollCapture } from './scroll/capture';
import { actionable, centreTarget, makeTarget } from './selector';
import { changeState, editableTracker, isSensitive, opensPicker } from './forms/capture';

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
  emit(event: SyncEvent): void;
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

interface PendingWheel {
  element: Element;
  x: number;
  y: number;
  deltaX: number;
  deltaY: number;
  deltaMode: number;
  modifiers: number;
}

/** Installs the listeners on `win`; the result removes them again. */
export function installCapture(win: Window, host: CaptureHost): () => void {
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

  const handlers: Record<string, (event: Event) => void> = {
    pointerdown: (event) => pointer(event as PointerEvent, 'down'),
    pointerup: (event) => pointer(event as PointerEvent, 'up'),
    click: (event) => mouse(event as MouseEvent, 'click'),
    dblclick: (event) => mouse(event as MouseEvent, 'dblclick'),
    contextmenu: (event) => mouse(event as MouseEvent, 'contextmenu'),
    pointermove: (event) => {
      const move = event as PointerEvent;
      if (move.pointerType === 'touch' || !host.enabled('hover')) return;
      const element = deepTarget(move);
      if (!element || element === lastRaw) return;
      lastRaw = element;
      const subject = actionable(element);
      if (subject === lastHover) return;
      lastHover = subject;
      hoverPending = { element: subject, x: move.clientX, y: move.clientY };
      const wait = hoverAt + host.hoverThrottleMs() - Date.now();
      if (wait <= 0) flushHover();
      else if (!hoverTimer) {
        hoverTimer = true;
        later(wait, flushHover);
      }
    },
    wheel: (event) => {
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
    },
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
    if (!element || !host.enabled('click')) return;
    // A native picker would open in every view; the control's value follows with `change`.
    if (type !== 'contextmenu' && opensPicker(element)) return;
    host.emit({
      type,
      button: buttonOf(event.button),
      detail: event.detail,
      modifiers: modifiers(event),
      target: makeTarget(element, event.clientX, event.clientY),
    });
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

  const listeners = Object.entries(handlers).map(([type, handle]) => {
    const listener = (event: Event) => {
      if (host.trusted(event)) handle(event);
    };
    win.addEventListener(type, listener, { capture: true, passive: true });
    return [type, listener] as const;
  });

  return () => {
    for (const [type, listener] of listeners) {
      win.removeEventListener(type, listener, { capture: true });
    }
    for (const timer of timers) clearTimeout(timer);
    timers.clear();
    scrolls.dispose();
  };
}
