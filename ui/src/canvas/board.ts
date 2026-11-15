// The canvas board's logic (RB-080-04), without the DOM: the drag of a label or of the empty
// board (one request per animation frame, then the end or Escape's cancel), the arrow keys, and
// what the board draws from the state.

import type { CanvasRequest, MapRect } from '../ipc/layout';
import type { EngineKind, ExternalView, StateSnapshot, ViewId } from '../ipc/protocol';

/** A press that moves less than this many px is a click. */
export const CLICK_SLOP = 3;

/** What a drag moves: one view by its label, or the canvas (a pan). */
export type DragTarget = { kind: 'view'; view: ViewId } | { kind: 'pan' };

/** A drag under way: the delta in whole px from where it began. */
export interface DragState {
  target: DragTarget;
  dx: number;
  dy: number;
  /** Past the click slop: the core has heard of it. */
  moved: boolean;
}

export interface BoardDragDeps {
  send: (request: CanvasRequest) => void;
  /** `requestAnimationFrame`. */
  frame: (callback: () => void) => number;
  /** `cancelAnimationFrame`. */
  cancelFrame: (handle: number) => void;
}

function request(state: DragState, phase: 'move' | 'end' | 'cancel'): CanvasRequest {
  const { dx, dy } = state;
  return state.target.kind === 'view'
    ? { type: 'canvasDrag', view: state.target.view, dx, dy, phase }
    : { type: 'canvasPan', dx, dy, phase };
}

/** One drag at a time; the live moves are throttled to the animation frames. */
export function createBoardDrag(deps: BoardDragDeps) {
  let active: (DragState & { startX: number; startY: number }) | null = null;
  let pending: number | null = null;

  const unschedule = (): void => {
    if (pending !== null) {
      deps.cancelFrame(pending);
      pending = null;
    }
  };

  const follow = (x: number, y: number): void => {
    if (!active) return;
    active.dx = Math.round(x - active.startX);
    active.dy = Math.round(y - active.startY);
    active.moved ||= Math.hypot(active.dx, active.dy) >= CLICK_SLOP;
  };

  const current = (): DragState | null =>
    active ? { target: active.target, dx: active.dx, dy: active.dy, moved: active.moved } : null;

  return {
    start(target: DragTarget, x: number, y: number): void {
      unschedule();
      active = { target, dx: 0, dy: 0, moved: false, startX: x, startY: y };
    },

    /** The pointer moved; the core hears of it on the next frame. */
    move(x: number, y: number): DragState | null {
      follow(x, y);
      if (active?.moved && pending === null) {
        pending = deps.frame(() => {
          pending = null;
          if (active?.moved) deps.send(request(active, 'move'));
        });
      }
      return current();
    },

    /** Let go: `click` below the slop (nothing sent), `dragged` after the end was sent. */
    end(x: number, y: number): 'click' | 'dragged' | null {
      if (!active) return null;
      unschedule();
      follow(x, y);
      const done = active;
      active = null;
      if (!done.moved) return 'click';
      deps.send(request(done, 'end'));
      return 'dragged';
    },

    /** Escape or a lost pointer: back to where it began. `false` without a drag. */
    cancel(): boolean {
      if (!active) return false;
      unschedule();
      const done = active;
      active = null;
      if (done.moved) deps.send(request(done, 'cancel'));
      return true;
    },

    current,
  };
}

const STEPS: Record<string, { dx: number; dy: number }> = {
  ArrowLeft: { dx: -1, dy: 0 },
  ArrowRight: { dx: 1, dy: 0 },
  ArrowUp: { dx: 0, dy: -1 },
  ArrowDown: { dx: 0, dy: 1 },
};

/** The step of an arrow key on a label; `null` for any other key. */
export function stepFor(key: string): { dx: number; dy: number } | null {
  return STEPS[key] ?? null;
}

/** How far `view` is drawn from its place while `drag` is under way (or settling). */
export function offsetOf(view: ViewId, drag: DragState | null): { x: number; y: number } {
  if (!drag?.moved) return { x: 0, y: 0 };
  const shifted = drag.target.kind === 'pan' || drag.target.view === view;
  return shifted ? { x: drag.dx, y: drag.dy } : { x: 0, y: 0 };
}

/** One view as the board draws it. */
export interface BoardEntry {
  view: ViewId;
  /** The label strip's text: the breakpoint, its size and pixel ratio, as the grid's strips. */
  label: string;
  labelRect: MapRect;
  rect: MapRect;
  visible: boolean;
  active: boolean;
  /** The session's badge colour; absent for `default`. */
  color: string | undefined;
  /** The view's engine (RB-080-33). */
  engine: EngineKind;
  /** The engine badge's text: the engine's name. */
  engineLabel: string;
  /** Why the engine cannot run (the badge is dimmed); `null` while it runs or starts. */
  engineReason: string | null;
}

/** The engines' names for the badge (proper nouns, not translated). */
const ENGINE_LABELS: Record<EngineKind, string> = { chromium: 'Chromium', firefox: 'Firefox', webkit: 'WebKit' };

/** Why the engine of `view` cannot run, as the core's external views say; `null` otherwise. */
export function engineReasonOf(view: ViewId, externals: ExternalView[] | undefined): string | null {
  const external = externals?.find((candidate) => candidate.view === view);
  return external?.status === 'unavailable' ? (external.reason ?? '') : null;
}

/** What the board draws: each view of `state`'s board that still exists, in grid order. */
export function boardItems(state: StateSnapshot | null): BoardEntry[] {
  const board = state?.layout.modes?.board;
  if (!state || !board) return [];
  return board.items.flatMap((item) => {
    const view = state.views.find((candidate) => candidate.id === item.view);
    if (!view) return [];
    const color = state.sessions.find((session) => session.id === view.session)?.color;
    return [
      {
        view: item.view,
        label: item.text,
        labelRect: item.label,
        rect: item.rect,
        visible: item.visible,
        active: state.activeView === item.view,
        color,
        engine: view.engine,
        engineLabel: ENGINE_LABELS[view.engine],
        engineReason: engineReasonOf(item.view, state.externals),
      },
    ];
  });
}
