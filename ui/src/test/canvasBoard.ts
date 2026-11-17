// The canvas board mounted with a shell whose state has a board of two views (RB-080-04), and
// the events its tests send; shared by `CanvasBoard.test.ts` and `CanvasBoardLabel.test.ts`.
import { vi } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import CanvasBoard from '../canvas/CanvasBoard.vue';
import { mountWithShell, settle } from './mountShell';
import type { BoardState } from '../ipc/layout';

let current: VueWrapper | null = null;

export async function setup(
  spacePan = true,
  options: {
    settings?: Record<string, unknown>;
    firefoxMissing?: boolean;
    hidden?: string[];
    wheelZoom?: boolean;
    pageZoom?: number;
  } = {},
) {
  vi.stubGlobal('requestAnimationFrame', (callback: () => void) => setTimeout(callback, 0) as unknown as number);
  vi.stubGlobal('cancelAnimationFrame', (handle: number) => clearTimeout(handle));
  const mounted = await mountWithShell(CanvasBoard, { mock: { settings: options.settings ?? {} } });
  current = mounted.wrapper;
  const [first, second] = mounted.mock.state.views;
  if (options.firefoxMissing) {
    first!.engine = 'firefox';
    mounted.mock.state.externals = [
      {
        view: first!.id,
        label: first!.label,
        breakpoint: 'mobile',
        engine: 'firefox',
        status: 'unavailable',
        reason: 'Firefox was not found',
        url: '',
      },
    ];
  }
  const board: BoardState = {
    spacePan,
    wheelZoom: options.wheelZoom ?? true,
    items: [
      {
        view: first!.id,
        label: { x: 0, y: 0, width: 200, height: 20 },
        rect: { x: 0, y: 20, width: 200, height: 400 },
        visible: true,
        text: `${first!.label}  400 × 800`,
        ...(options.pageZoom === undefined ? {} : { pageZoom: options.pageZoom }),
      },
      {
        view: second!.id,
        label: { x: 216, y: 0, width: 300, height: 20 },
        rect: { x: 216, y: 20, width: 300, height: 400 },
        visible: false,
        text: `${second!.label}  768 × 1024 @2x`,
      },
    ],
  };
  mounted.mock.state.layout = {
    ...mounted.mock.state.layout,
    mode: 'canvas',
    modes: { board: { ...board, camera: { x: -40, y: 12, zoomPermille: 500 } } },
    hidden: options.hidden ?? [],
  };
  mounted.mock.emit({ type: 'state', ...structuredClone(mounted.mock.state) });
  await settle();
  return { ...mounted, first: first!, second: second! };
}

/** Unmounts the board and gives the clock and the globals back; each test file's `afterEach`. */
export function cleanup(): void {
  current?.unmount();
  current = null;
  vi.useRealTimers();
  vi.unstubAllGlobals();
}

export function pointer(type: string, x: number, y: number, buttons = type === 'pointerup' ? 0 : 1): PointerEvent {
  // happy-dom's PointerEvent may lack coordinates; a MouseEvent carries them.
  return new MouseEvent(type, { clientX: x, clientY: y, bubbles: true, button: 0, buttons }) as PointerEvent;
}

// The test DOM's WheelEvent drops the mouse fields: a mouse event of type `wheel` carries them.
export function wheelEvent(deltaY: number, init: MouseEventInit = {}): WheelEvent {
  const event = new MouseEvent('wheel', { bubbles: true, cancelable: true, ...init });
  Object.defineProperty(event, 'deltaY', { value: deltaY });
  return event as WheelEvent;
}

export function sentOf(mock: { sent: unknown[] }, type: string): unknown[] {
  return mock.sent.filter((message) => (message as { type: string }).type === type);
}
