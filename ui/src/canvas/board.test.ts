import { describe, expect, it } from 'vitest';
import type { CanvasRequest } from '../ipc/layout';
import { sampleState } from '../ipc/mockState';
import type { StateSnapshot } from '../ipc/protocol';
import { badgeState, badgeTitle, boardItems, createBoardDrag, engineReasonOf, labelPieces, stepFor } from './board';

/** A board drag whose sends the core answers when the test says so, and its frames. */
function harness() {
  const sent: CanvasRequest[] = [];
  const answers: ((ok: boolean) => void)[] = [];
  const frames: (() => void)[] = [];
  const drag = createBoardDrag({
    send: (request) => {
      sent.push(request);
      return new Promise<boolean>((resolve) => answers.push(resolve));
    },
    frame: (callback) => frames.push(callback),
    cancelFrame: (handle) => {
      frames[handle - 1] = () => undefined;
    },
  });
  const flush = (): void => {
    frames.splice(0).forEach((callback) => callback());
  };
  /** The core answers every request sent so far. */
  const answer = async (): Promise<void> => {
    answers.splice(0).forEach((resolve) => resolve(true));
    await Promise.resolve();
    await Promise.resolve();
  };
  return { sent, drag, flush, answer };
}

describe('a drag on the canvas board', () => {
  // RB-094-18: the board sent a move every frame, one frame late, with nothing in step with the
  // core - the core's frames and the window's moves queued up. The board now sends the newest
  // delta with the next frame, never a second one before the core answered the first.
  it('sends the newest delta with the next frame, one move at a time, then the end', async () => {
    const { sent, drag, flush, answer } = harness();
    drag.start({ kind: 'view', view: 2 }, 100, 100);
    drag.move(104, 101);
    drag.move(110, 95.4);
    flush();
    expect(sent).toEqual([{ type: 'canvasDrag', view: 2, dx: 10, dy: -5, phase: 'move' }]);
    drag.move(120, 92);
    drag.move(125, 90);
    flush();
    expect(sent, 'nothing while the core has not answered').toHaveLength(1);
    await answer();
    flush();
    // The newest delta; the one in between is never sent.
    expect(sent.slice(1)).toEqual([{ type: 'canvasDrag', view: 2, dx: 25, dy: -10, phase: 'move' }]);
    await answer();
    flush();
    expect(sent, 'nothing new, nothing sent').toHaveLength(2);
    drag.move(130, 90);
    expect(drag.end(140, 80)).toBe('dragged');
    flush();
    // The end goes at once, and no move after it.
    expect(sent.slice(2)).toEqual([{ type: 'canvasDrag', view: 2, dx: 40, dy: -20, phase: 'end' }]);
    await answer();
    flush();
    expect(sent).toHaveLength(3);
    expect(drag.current()).toBeNull();
  });

  it('sends the end at once while a move is still unanswered, and nothing after it', async () => {
    const { sent, drag, flush, answer } = harness();
    drag.start({ kind: 'view', view: 2 }, 0, 0);
    drag.move(10, 0);
    flush();
    drag.move(20, 0);
    expect(drag.end(30, 0)).toBe('dragged');
    await answer();
    flush();
    expect(sent.map((request) => request.type === 'canvasDrag' && request.phase)).toEqual(['move', 'end']);
  });

  it('starts the next drag without waiting for an answer of the last one', async () => {
    const { sent, drag, flush, answer } = harness();
    drag.start({ kind: 'pan' }, 0, 0);
    drag.move(10, 0);
    flush();
    drag.end(10, 0);
    drag.start({ kind: 'pan' }, 0, 0);
    drag.move(0, 10);
    flush();
    expect(sent.at(-1)).toEqual({ type: 'canvasPan', dx: 0, dy: 10, phase: 'move' });
    // The first drag's answer arrives late: it sends nothing of its own.
    await answer();
    flush();
    expect(sent).toHaveLength(3);
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
    // An iOS view's badge says iOS (RB-095-04).
    first.engine = 'ios';
    expect(boardItems(state)[0]).toMatchObject({ engine: 'ios', engineLabel: 'iOS' });
  });
});
