import { describe, expect, it } from 'vitest';
import type { CanvasRequest } from '../ipc/layout';
import { sampleState } from '../ipc/mockState';
import type { StateSnapshot } from '../ipc/protocol';
import { badgeState, badgeTitle, boardItems, createBoardDrag, engineReasonOf, labelPieces, offsetOf, stepFor } from './board';

function harness() {
  const sent: CanvasRequest[] = [];
  const frames: (() => void)[] = [];
  const drag = createBoardDrag({
    send: (request) => sent.push(request),
    frame: (callback) => frames.push(callback),
    cancelFrame: (handle) => {
      frames[handle - 1] = () => undefined;
    },
  });
  const flush = (): void => {
    frames.splice(0).forEach((callback) => callback());
  };
  return { sent, drag, flush };
}

describe('a drag on the canvas board', () => {
  it('sends one move per frame with the latest delta, then the end', () => {
    const { sent, drag, flush } = harness();
    drag.start({ kind: 'view', view: 2 }, 100, 100);
    drag.move(104, 101);
    drag.move(110, 95.4);
    expect(sent).toEqual([]);
    flush();
    expect(sent).toEqual([{ type: 'canvasDrag', view: 2, dx: 10, dy: -5, phase: 'move' }]);
    drag.move(130, 90);
    expect(drag.end(140, 80)).toBe('dragged');
    flush();
    expect(sent.slice(1)).toEqual([{ type: 'canvasDrag', view: 2, dx: 40, dy: -20, phase: 'end' }]);
    expect(drag.current()).toBeNull();
  });

  it('is a click below the slop and sends nothing', () => {
    const { sent, drag, flush } = harness();
    drag.start({ kind: 'view', view: 2 }, 10, 10);
    drag.move(11, 12);
    expect(drag.end(11, 12)).toBe('click');
    flush();
    expect(sent).toEqual([]);
  });

  it('pans the empty board and cancels with Escape', () => {
    const { sent, drag, flush } = harness();
    drag.start({ kind: 'pan' }, 0, 0);
    drag.move(-30, 12);
    flush();
    expect(drag.cancel()).toBe(true);
    flush();
    expect(sent).toEqual([
      { type: 'canvasPan', dx: -30, dy: 12, phase: 'move' },
      { type: 'canvasPan', dx: -30, dy: 12, phase: 'cancel' },
    ]);
    expect(drag.cancel()).toBe(false);
    expect(drag.end(0, 0)).toBeNull();
  });

  it('cancels a click-sized press without a word to the core', () => {
    const { sent, drag } = harness();
    drag.start({ kind: 'view', view: 1 }, 0, 0);
    expect(drag.cancel()).toBe(true);
    expect(sent).toEqual([]);
  });
});

describe('the board', () => {
  it('moves the arrow keys by one step', () => {
    expect(stepFor('ArrowLeft')).toEqual({ dx: -1, dy: 0 });
    expect(stepFor('ArrowDown')).toEqual({ dx: 0, dy: 1 });
    expect(stepFor('a')).toBeNull();
  });

  it('shifts the dragged view or, panning, every view', () => {
    const view = { target: { kind: 'view', view: 2 }, dx: 5, dy: 6, moved: true } as const;
    expect(offsetOf(2, view)).toEqual({ x: 5, y: 6 });
    expect(offsetOf(3, view)).toEqual({ x: 0, y: 0 });
    expect(offsetOf(3, { target: { kind: 'pan' }, dx: -1, dy: 2, moved: true })).toEqual({ x: -1, y: 2 });
    expect(offsetOf(3, { target: { kind: 'pan' }, dx: -1, dy: 2, moved: false })).toEqual({ x: 0, y: 0 });
    expect(offsetOf(3, null)).toEqual({ x: 0, y: 0 });
  });

  it('lists each view with its strip text, session colour and whether it is active', () => {
    const state: StateSnapshot = structuredClone(sampleState());
    const [first, second] = state.views;
    second!.session = 'admin';
    state.activeView = second!.id;
    state.layout.modes = {
      board: {
        spacePan: true,
        items: [
          {
            view: first!.id,
            label: { x: 0, y: 0, width: 200, height: 20 },
            rect: { x: 0, y: 20, width: 200, height: 400 },
            visible: true,
            text: 'Mobile  400 × 800',
          },
          {
            view: second!.id,
            label: { x: 216, y: 0, width: 300, height: 20 },
            rect: { x: 216, y: 20, width: 300, height: 400 },
            visible: false,
            text: 'Tablet  768 × 1024 @2x',
          },
          {
            view: 999,
            label: { x: 0, y: 0, width: 1, height: 1 },
            rect: { x: 0, y: 0, width: 1, height: 1 },
            visible: true,
            text: 'Gone',
          },
        ],
      },
    };
    const items = boardItems(state);
    expect(items.map((item) => item.view)).toEqual([first!.id, second!.id]);
    expect(items[0]).toMatchObject({
      label: 'Mobile  400 × 800',
      active: false,
      color: undefined,
      engine: 'chromium',
      engineLabel: 'Chromium',
      engineReason: null,
    });
    expect(items[1]).toMatchObject({ label: 'Tablet  768 × 1024 @2x', active: true, color: '#e5484d', visible: false });
    expect(boardItems(null)).toEqual([]);
  });
});

// RB-090-69: a label shows the pieces a grid strip shows beside the name, in the strip's order -
// the address, the Web Vitals, the external engine's window - each only while the core sends it.
describe('the pieces of a board label', () => {
  it('lists the pieces the core sent, in the strip order, and none without them', () => {
    const state: StateSnapshot = structuredClone(sampleState());
    const [first, second] = state.views;
    const rect = { x: 0, y: 0, width: 200, height: 20 };
    state.layout.modes = {
      board: {
        spacePan: true,
        items: [
          {
            view: first!.id,
            label: rect,
            rect,
            visible: true,
            text: 'Mobile',
            engineStatus: { text: 'Firefox: closed', tooltip: 'Firefox closed its window' },
            address: { text: 'localhost:3000/a', tooltip: 'Address of Mobile\nhttp://localhost:3000/a' },
          },
          { view: second!.id, label: rect, rect, visible: true, text: 'Tablet' },
        ],
      },
    };
    const [one, two] = boardItems(state);
    expect(one!.pieces).toEqual([
      { kind: 'address', text: 'localhost:3000/a', tooltip: 'Address of Mobile\nhttp://localhost:3000/a' },
      { kind: 'engineStatus', text: 'Firefox: closed', tooltip: 'Firefox closed its window' },
    ]);
    expect(two!.pieces).toEqual([]);
  });

  // RB-090-71: while the engine badge shows, the engine's state stands in it - one badge, not
  // two; without the badge it stays a piece of the label.
  it('moves the engine state into the engine badge while the badge shows', () => {
    const address = { kind: 'address' as const, text: 'localhost:3000/a', tooltip: 'Address' };
    const state = { kind: 'engineStatus' as const, text: 'Firefox · window closed', tooltip: 'Closed' };
    expect(labelPieces([address, state], true)).toEqual([address]);
    expect(labelPieces([address, state], false)).toEqual([address, state]);
    expect(badgeState([address, state], true)).toEqual(state);
    expect(badgeState([address, state], false)).toBeNull();
    expect(badgeState([address], true)).toBeNull();
  });

  // RB-090-71 review: a badge whose engine cannot run names its reason once, and keeps its
  // dimmed look; a running one puts the state's details first.
  it('words the badge title by whether the engine can run', () => {
    const state = { kind: 'engineStatus' as const, text: 'Firefox · window closed', tooltip: 'Closed.' };
    expect(badgeTitle('Click to choose.', false, state)).toBe('Closed.\nClick to choose.');
    expect(badgeTitle('Firefox cannot run: not found.', true, state)).toBe('Firefox cannot run: not found.');
    expect(badgeTitle('Click to choose.', false, null)).toBe('Click to choose.');
  });
});

describe('the engine badge of a board label', () => {
  it('names the engine and says why it cannot run', () => {
    const state = sampleState();
    const first = state.views[0]!;
    first.engine = 'firefox';
    state.externals = [
      {
        view: first.id,
        label: first.label,
        breakpoint: 'mobile',
        engine: 'firefox',
        status: 'unavailable',
        reason: 'Firefox was not found',
        url: '',
      },
    ];
    state.layout = {
      ...state.layout,
      mode: 'canvas',
      modes: {
        board: {
          spacePan: true,
          items: [
            {
              view: first.id,
              label: { x: 0, y: 0, width: 200, height: 20 },
              rect: { x: 0, y: 20, width: 200, height: 400 },
              visible: true,
              text: 'Mobile  400 × 800',
            },
          ],
        },
      },
    };
    const [item] = boardItems(state);
    expect(item).toMatchObject({ engine: 'firefox', engineLabel: 'Firefox', engineReason: 'Firefox was not found' });
    expect(engineReasonOf(first.id, undefined)).toBeNull();
    expect(engineReasonOf(first.id, [{ ...state.externals[0]!, status: 'running' }])).toBeNull();
  });
});
