// The canvas zoom of the status bar (RB-080-94): while the canvas layout is shown, the active
// view's page zoom (camera x its item zoom; 100 % shows its page at its real size), from the
// state's board (`levelPermille`) and from the board a gesture is trying out (`canvasLive`).
// Every change is a command of the one registry: `canvas.zoom.reset { level }` for the slider,
// `canvas.zoom.in` / `canvas.zoom.out` for the buttons. The presets (Fit, 25 to 200 %) are the
// core's native menu (`canvasZoomMenu`, RB-090-18): a list the page drew or a `<select>` opened
// below the bar, off the screen at its bottom. `ui.statusBar.canvasZoom` switches it off.

import { computed, ref } from 'vue';
import type { IpcClient } from '../ipc/client';
import type { BoardState } from '../ipc/layout';
import type { CoreToShell, ShellToCore } from '../ipc/protocol';

/** The setting that shows it. */
export const CANVAS_ZOOM_KEY = 'ui.statusBar.canvasZoom';
/** The slider's ends and step, in per cent: the core's levels - the lowest is the camera's
 *  lowest zoom on the smallest item zoom (`canvas_level::MIN_LEVEL`, 0.5 %), the highest
 *  `canvas::MAX_PAGE_ZOOM` - so the slider always stands where the canvas is and moving it
 *  never jumps. */
export const MIN_PERCENT = 0.5;
export const MAX_PERCENT = 200;
export const STEP_PERCENT = 0.5;
/** The page zoom of `board` in per cent: whole from 10 % on, a tenth below (2.5 %, 0.5 %), never
 *  rounded to 0; `null` without a canvas or a level. */
export function percentOf(board: BoardState | null | undefined): number | null {
  const level = board?.levelPermille;
  if (level === undefined || level <= 0) return null;
  return level >= 100 ? Math.round(level / 10) : level / 10;
}

/** `percent` held to the slider's range and step. */
export function clampPercent(percent: number): number {
  const stepped = Math.round(percent / STEP_PERCENT) * STEP_PERCENT;
  return Math.min(MAX_PERCENT, Math.max(MIN_PERCENT, stepped));
}

/** The request that opens the presets: the core's native menu with its bottom centre at the
 *  middle of `box`'s top edge (DIP from the status bar's corner), so it opens above the bar. */
export function presetsMenuRequest(box: Pick<DOMRect, 'left' | 'top' | 'width'>): ShellToCore {
  return { type: 'canvasZoomMenu', x: Math.round(box.left + box.width / 2), y: Math.round(box.top) };
}

export type CanvasZoom = ReturnType<typeof createCanvasZoom>;

export function createCanvasZoom(client: IpcClient) {
  const board = ref<BoardState | null>(null);
  const enabled = ref(true);
  const stops: (() => void)[] = [];

  const percent = computed(() => percentOf(board.value));
  const visible = computed(() => enabled.value && percent.value !== null);

  function send(message: ShellToCore): void {
    client.request(message).catch((error: unknown) => {
      console.warn('the canvas zoom could not reach the core', message.type, error);
    });
  }

  async function loadSettings(): Promise<void> {
    try {
      const answer = await client.expect({ type: 'getSettings' }, 'settings');
      enabled.value = answer.values.find((entry) => entry.key === CANVAS_ZOOM_KEY)?.value !== false;
    } catch {
      // The default stays: shown.
    }
  }

  const onEvent = (message: CoreToShell): void => {
    if (message.type === 'state') {
      board.value = message.layout.modes?.board ?? null;
    } else if (message.type === 'canvasLive') {
      board.value = message.board;
    } else if (message.type === 'settingsChanged') {
      void loadSettings();
    }
  };

  async function start(): Promise<void> {
    stops.push(client.on(onEvent));
    try {
      const state = await client.expect({ type: 'getState' }, 'state');
      board.value = state.layout.modes?.board ?? null;
    } catch {
      // The next state event brings it.
    }
    await loadSettings();
  }

  function stop(): void {
    stops.splice(0).forEach((fn) => fn());
  }

  return {
    board,
    enabled,
    percent,
    visible,
    start,
    stop,
    /** The slider let go at `value` per cent. */
    setPercent: (value: number) => send({ type: 'runCommand', id: 'canvas.zoom.reset', args: { level: clampPercent(value) / 100 } }),
    step: (up: boolean) => send({ type: 'runCommand', id: up ? 'canvas.zoom.in' : 'canvas.zoom.out' }),
    /** The presets' native menu above `box`, the level button's place. */
    openPresets: (box: Pick<DOMRect, 'left' | 'top' | 'width'>) => send(presetsMenuRequest(box)),
  };
}
