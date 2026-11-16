import { afterEach, describe, expect, it, vi } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { mountWithShell, settle } from '../test/mountShell';
import CanvasBoard from './CanvasBoard.vue';
import type { BoardState } from '../ipc/layout';

let current: VueWrapper | null = null;

async function setup(
  spacePan = true,
  options: { settings?: Record<string, unknown>; firefoxMissing?: boolean; hidden?: string[]; wheelZoom?: boolean } = {},
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

afterEach(() => {
  current?.unmount();
  current = null;
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

function pointer(type: string, x: number, y: number, buttons = type === 'pointerup' ? 0 : 1): PointerEvent {
  // happy-dom's PointerEvent may lack coordinates; a MouseEvent carries them.
  return new MouseEvent(type, { clientX: x, clientY: y, bubbles: true, button: 0, buttons }) as PointerEvent;
}

// The test DOM's WheelEvent drops the mouse fields: a mouse event of type `wheel` carries them.
function wheelEvent(deltaY: number, init: MouseEventInit = {}): WheelEvent {
  const event = new MouseEvent('wheel', { bubbles: true, cancelable: true, ...init });
  Object.defineProperty(event, 'deltaY', { value: deltaY });
  return event as WheelEvent;
}

function sentOf(mock: { sent: unknown[] }, type: string): unknown[] {
  return mock.sent.filter((message) => (message as { type: string }).type === type);
}

describe('the canvas board', () => {
  it('draws each view label with its size at its place, focusable, the hidden one marked', async () => {
    const { wrapper, first, second } = await setup();
    const labels = wrapper.findAll('[data-testid^="board-label-"]');
    expect(labels).toHaveLength(2);
    const one = wrapper.get(`[data-testid="board-label-${first.id}"]`);
    expect(one.attributes('tabindex')).toBe('0');
    expect(one.attributes('role')).toBe('button');
    expect(one.get('span.truncate').text()).toBe(`${first.label}  400 × 800`);
    expect(wrapper.get(`[data-testid="board-strip-${first.id}"]`).attributes('style')).toContain('left: 0px');
    const two = wrapper.get(`[data-testid="board-label-${second.id}"]`);
    expect(wrapper.get(`[data-testid="board-strip-${second.id}"]`).attributes('style')).toContain('left: 216px');
    expect(two.attributes('aria-label')).toBe(`${second.label}  768 × 1024 @2x, outside the visible area`);
  });

  it('moves a dragged label live and lets go with the end', async () => {
    const { wrapper, mock, first } = await setup();
    const label = wrapper.get(`[data-testid="board-label-${first.id}"]`).element;
    label.dispatchEvent(pointer('pointerdown', 10, 10));
    label.dispatchEvent(pointer('pointermove', 40, 30));
    await settle();
    expect(sentOf(mock, 'canvasDrag')).toEqual([
      { type: 'canvasDrag', view: first.id, dx: 30, dy: 20, phase: 'move' },
    ]);
    expect(wrapper.get(`[data-testid="board-strip-${first.id}"]`).attributes('style')).toContain(
      'translate(30px, 20px)',
    );
    label.dispatchEvent(pointer('pointerup', 50, 30));
    await settle();
    expect(sentOf(mock, 'canvasDrag').at(-1)).toEqual({
      type: 'canvasDrag',
      view: first.id,
      dx: 40,
      dy: 20,
      phase: 'end',
    });
    expect(sentOf(mock, 'setActiveView')).toEqual([]);
  });

  // Moving the view's overlay may take the focus from the board (the window activates it); the
  // drag goes on and ends where the pointer is let go (owner, diag5: it snapped back).
  it('keeps a drag going when the board loses the focus', async () => {
    const { wrapper, mock, first } = await setup();
    const label = wrapper.get(`[data-testid="board-label-${first.id}"]`).element;
    label.dispatchEvent(pointer('pointerdown', 10, 10));
    label.dispatchEvent(pointer('pointermove', 40, 30));
    await settle();
    window.dispatchEvent(new Event('blur'));
    label.dispatchEvent(pointer('pointermove', 60, 30));
    label.dispatchEvent(pointer('pointerup', 60, 30));
    await settle();
    const phases = sentOf(mock, 'canvasDrag').map((message) => (message as { phase: string }).phase);
    expect(phases).not.toContain('cancel');
    expect(sentOf(mock, 'canvasDrag').at(-1)).toEqual({
      type: 'canvasDrag',
      view: first.id,
      dx: 50,
      dy: 20,
      phase: 'end',
    });
  });

  it('ends a drag at a move with no button held (let go where the board did not hear it)', async () => {
    const { wrapper, mock, first } = await setup();
    const label = wrapper.get(`[data-testid="board-label-${first.id}"]`).element;
    label.dispatchEvent(pointer('pointerdown', 10, 10));
    label.dispatchEvent(pointer('pointermove', 40, 30));
    label.dispatchEvent(pointer('pointermove', 45, 30, 0));
    await settle();
    expect(sentOf(mock, 'canvasDrag').at(-1)).toMatchObject({ view: first.id, dx: 35, phase: 'end' });
  });

  it('makes a clicked label the active view', async () => {
    const { wrapper, mock, second } = await setup();
    const label = wrapper.get(`[data-testid="board-label-${second.id}"]`).element;
    label.dispatchEvent(pointer('pointerdown', 220, 5));
    label.dispatchEvent(pointer('pointerup', 221, 5));
    await settle();
    expect(sentOf(mock, 'setActiveView')).toEqual([{ type: 'setActiveView', view: second.id }]);
    expect(sentOf(mock, 'canvasSizePick')).toEqual([]);
  });

  // The first view is the active one in the mock: a click on its label asks the core for the
  // size quick pick, as a click on the active view's strip does in the other modes (RB-080-22).
  it('asks for the size pick with a click on the active label, not after a drag', async () => {
    const { wrapper, mock, first } = await setup();
    const label = wrapper.get(`[data-testid="board-label-${first.id}"]`).element;
    label.dispatchEvent(pointer('pointerdown', 10, 5));
    label.dispatchEvent(pointer('pointerup', 11, 5));
    await settle();
    expect(sentOf(mock, 'canvasSizePick')).toEqual([{ type: 'canvasSizePick', view: first.id }]);
    expect(sentOf(mock, 'setActiveView')).toEqual([]);
    label.dispatchEvent(pointer('pointerdown', 10, 5));
    label.dispatchEvent(pointer('pointermove', 40, 5));
    label.dispatchEvent(pointer('pointerup', 40, 5));
    await settle();
    expect(sentOf(mock, 'canvasSizePick')).toHaveLength(1);
    expect(sentOf(mock, 'canvasDrag').at(-1)).toMatchObject({ view: first.id, dx: 30, phase: 'end' });
  });

  it('pans on the empty board and cancels with Escape', async () => {
    const { wrapper, mock } = await setup();
    const area = wrapper.get('[data-testid="board"]').element;
    area.dispatchEvent(pointer('pointerdown', 600, 600));
    area.dispatchEvent(pointer('pointermove', 580, 640));
    await settle();
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    await settle();
    expect(sentOf(mock, 'canvasPan')).toEqual([
      { type: 'canvasPan', dx: -20, dy: 40, phase: 'move' },
      { type: 'canvasPan', dx: -20, dy: 40, phase: 'cancel' },
    ]);
  });

  it('pans from a label while Space is held, when the setting allows', async () => {
    const { wrapper, mock, first } = await setup();
    window.dispatchEvent(new KeyboardEvent('keydown', { key: ' ' }));
    const label = wrapper.get(`[data-testid="board-label-${first.id}"]`).element;
    label.dispatchEvent(pointer('pointerdown', 10, 10));
    label.dispatchEvent(pointer('pointerup', 30, 10));
    window.dispatchEvent(new KeyboardEvent('keyup', { key: ' ' }));
    await settle();
    expect(sentOf(mock, 'canvasPan').at(-1)).toEqual({ type: 'canvasPan', dx: 20, dy: 0, phase: 'end' });
    expect(sentOf(mock, 'canvasDrag')).toEqual([]);
  });

  it('drags the view with Space held when Space pan is off', async () => {
    const { wrapper, mock, first } = await setup(false);
    window.dispatchEvent(new KeyboardEvent('keydown', { key: ' ' }));
    const label = wrapper.get(`[data-testid="board-label-${first.id}"]`).element;
    label.dispatchEvent(pointer('pointerdown', 10, 10));
    label.dispatchEvent(pointer('pointerup', 30, 10));
    await settle();
    expect(sentOf(mock, 'canvasPan')).toEqual([]);
    expect(sentOf(mock, 'canvasDrag').at(-1)).toMatchObject({ view: first.id, dx: 20, phase: 'end' });
  });

  it('moves a focused label with the arrow keys and activates it with Enter', async () => {
    const { wrapper, mock, first } = await setup();
    const label = wrapper.get(`[data-testid="board-label-${first.id}"]`);
    await label.trigger('keydown', { key: 'ArrowUp' });
    await label.trigger('keydown', { key: 'Enter' });
    await settle();
    expect(sentOf(mock, 'canvasStep')).toEqual([{ type: 'canvasStep', view: first.id, dx: 0, dy: -1 }]);
    expect(sentOf(mock, 'setActiveView')).toEqual([{ type: 'setActiveView', view: first.id }]);
  });

  // RB-090-23 (WCAG 4.1.2): a button holds no button - the label and the engine badge are two
  // buttons side by side in the strip; Space activates the label as Enter does.
  it('keeps the label button and the engine badge apart, and activates the label with Space', async () => {
    const { wrapper, mock, first } = await setup(false);
    const label = wrapper.get(`[data-testid="board-label-${first.id}"]`);
    expect(label.find('button').exists()).toBe(false);
    const strip = wrapper.get(`[data-testid="board-strip-${first.id}"]`);
    expect(strip.attributes('role')).toBeUndefined();
    expect(strip.find(`[data-testid="board-engine-${first.id}"]`).exists()).toBe(true);
    await label.trigger('keydown', { key: ' ' });
    await label.trigger('keyup', { key: ' ' });
    await settle();
    expect(sentOf(mock, 'setActiveView')).toEqual([{ type: 'setActiveView', view: first.id }]);
  });

  it('activates nothing when the Space held on a label panned the canvas', async () => {
    const { wrapper, mock, first } = await setup(true);
    const label = wrapper.get(`[data-testid="board-label-${first.id}"]`);
    await label.trigger('keydown', { key: ' ' });
    label.element.dispatchEvent(pointer('pointerdown', 10, 10));
    label.element.dispatchEvent(pointer('pointermove', 40, 30));
    label.element.dispatchEvent(pointer('pointerup', 40, 30));
    await label.trigger('keyup', { key: ' ' });
    await settle();
    expect(sentOf(mock, 'setActiveView')).toEqual([]);
  });

  it('ends each label in the engine badge, whose click asks for the engine menu and moves nothing', async () => {
    const { wrapper, mock, first, second } = await setup();
    const badge = wrapper.get(`[data-testid="board-engine-${second.id}"]`);
    expect(badge.text()).toBe('Chromium');
    expect(badge.attributes('data-available')).toBe('true');
    expect(badge.attributes('title')).toContain('Chromium');
    expect(wrapper.get(`[data-testid="board-engine-${first.id}"]`).text()).toBe('Chromium');
    badge.element.dispatchEvent(pointer('pointerdown', 500, 5));
    badge.element.dispatchEvent(pointer('pointerup', 501, 5));
    await badge.trigger('click');
    await settle();
    expect(sentOf(mock, 'canvasEnginePick')).toEqual([{ type: 'canvasEnginePick', view: second.id }]);
    expect(sentOf(mock, 'setActiveView')).toEqual([]);
    expect(sentOf(mock, 'canvasSizePick')).toEqual([]);
    expect(sentOf(mock, 'canvasDrag')).toEqual([]);
  });

  it('dims the badge of an engine that cannot run and names the reason', async () => {
    const { wrapper, first } = await setup(true, { firefoxMissing: true });
    const badge = wrapper.get(`[data-testid="board-engine-${first.id}"]`);
    expect(badge.text()).toBe('Firefox');
    expect(badge.attributes('data-available')).toBe('false');
    expect(badge.attributes('title')).toContain('Firefox was not found');
    expect(badge.classes()).toContain('opacity-60');
    // RB-090-23: not only dimmed - a warning mark, and the reason is in its name.
    expect(badge.find('[data-testid="board-engine-warning"]').exists()).toBe(true);
    expect(badge.attributes('aria-label')).toBe(
      `Engine of ${first.label}  400 × 800: Firefox, cannot run: Firefox was not found`,
    );
    // RB-090-23 (WCAG 2.5.8): the full strip height and at least 24 px wide, 12 px text.
    expect(badge.classes()).toEqual(expect.arrayContaining(['self-stretch', 'min-w-6', 'text-xs']));
  });

  // RB-080-46: the board is a browser view behind the views' native overlays, so a menu it draws
  // itself lies behind them. A right-click names the place; the core shows a native menu there.
  it('asks the core for the view menu on a right-clicked label and draws none itself', async () => {
    const { wrapper, mock, second } = await setup(true, { hidden: ['desktop'] });
    const label = wrapper.get(`[data-testid="board-label-${second.id}"]`).element;
    const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 230, clientY: 5, button: 2 });
    label.dispatchEvent(event);
    await settle();
    expect(document.body.querySelector('[role="menu"]'), 'no menu in the board\'s own page').toBeNull();
    expect(event.defaultPrevented, 'the page shows no menu of its own').toBe(true);
    expect(sentOf(mock, 'canvasMenu')).toEqual([{ type: 'canvasMenu', view: second.id, x: 230, y: 5 }]);
    expect(sentOf(mock, 'runCommand')).toEqual([]);
    expect(sentOf(mock, 'canvasDrag')).toEqual([]);
    expect(sentOf(mock, 'setActiveView')).toEqual([]);
  });

  it('asks the core for the add menu on the right-clicked free area, with the place', async () => {
    const { wrapper, mock } = await setup(true, { hidden: ['desktop', 'tablet'] });
    const area = wrapper.get('[data-testid="board"]').element;
    area.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 100.4, clientY: 50, button: 2 }));
    await settle();
    expect(document.body.querySelector('[role="menu"]')).toBeNull();
    // The core turns the place into a canvas point through its camera.
    expect(sentOf(mock, 'canvasMenu')).toEqual([{ type: 'canvasMenu', x: 100, y: 50 }]);
  });

  it('asks for no menu when ui.viewContextMenu is off', async () => {
    const { wrapper, mock, first } = await setup(true, { settings: { 'ui.viewContextMenu': false } });
    const label = wrapper.get(`[data-testid="board-label-${first.id}"]`).element;
    const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 5, clientY: 5, button: 2 });
    label.dispatchEvent(event);
    await settle();
    expect(sentOf(mock, 'canvasMenu')).toEqual([]);
    expect(event.defaultPrevented, 'the page shows no menu of its own').toBe(true);
  });

  // RB-080-50: the board heard no wheel at all, so Ctrl+wheel was Chromium's page zoom of the
  // shell's pages. The board takes it and asks the core to zoom the canvas around the pointer.
  it('zooms the canvas around the pointer on Ctrl+wheel instead of the page', async () => {
    const { wrapper, mock } = await setup();
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
    try {
      const area = wrapper.get('[data-testid="board"]').element;
      const event = wheelEvent(-100, { ctrlKey: true, clientX: 320.4, clientY: 200 });
      area.dispatchEvent(event);
      area.dispatchEvent(wheelEvent(-50, { ctrlKey: true, clientX: 322, clientY: 201 }));
      expect(event.defaultPrevented, 'the page does not zoom').toBe(true);
      await vi.advanceTimersByTimeAsync(20);
      // One request per frame: the whole turn since the gesture began, around where it began.
      expect(sentOf(mock, 'canvasZoom')).toEqual([{ type: 'canvasZoom', x: 320, y: 200, delta: -150, phase: 'move' }]);
      area.dispatchEvent(wheelEvent(-100, { ctrlKey: true, clientX: 330, clientY: 210 }));
      await vi.advanceTimersByTimeAsync(20);
      expect(sentOf(mock, 'canvasZoom').at(-1)).toEqual({ type: 'canvasZoom', x: 320, y: 200, delta: -250, phase: 'move' });
      // The wheel rests: the zoom is kept, once.
      await vi.advanceTimersByTimeAsync(200);
      expect(sentOf(mock, 'canvasZoom').at(-1)).toEqual({ type: 'canvasZoom', x: 320, y: 200, delta: -250, phase: 'end' });
      expect(sentOf(mock, 'canvasZoom')).toHaveLength(3);
      // The next turn is a gesture of its own.
      area.dispatchEvent(wheelEvent(100, { ctrlKey: true, clientX: 10, clientY: 10 }));
      await vi.advanceTimersByTimeAsync(20);
      expect(sentOf(mock, 'canvasZoom').at(-1)).toEqual({ type: 'canvasZoom', x: 10, y: 10, delta: 100, phase: 'move' });
      // A plain wheel is not the board's.
      const plain = wheelEvent(-100);
      area.dispatchEvent(plain);
      expect(plain.defaultPrevented).toBe(false);
    } finally {
      vi.useRealTimers();
    }
  });

  // Review of RB-080-50: while a gesture moves the canvas the core sends the board's frames for
  // the camera on trial; the next state is the truth again.
  it('draws the frames of a camera on trial until the next state', async () => {
    const { wrapper, mock, first } = await setup();
    const label = () => wrapper.get(`[data-testid="board-strip-${first.id}"]`).attributes('style');
    expect(label()).toContain('left: 0px');
    const board = structuredClone(mock.state.layout.modes?.board);
    board!.items[0]!.label.x = 37;
    mock.emit({ type: 'canvasLive', board: board!, seq: 1 });
    await settle();
    expect(label()).toContain('left: 37px');
    mock.emit({ type: 'state', ...structuredClone(mock.state) });
    await settle();
    expect(label()).toContain('left: 0px');
  });

  // RB-090-55: the core moved the views at once and the board drew their frames, labels and the
  // frame around the footer later. The board now says which canvas it painted - the newest
  // once, after drawing it - and the core moves the views then.
  it('draws a canvas on trial, frame around the footer included, and then says it painted it', async () => {
    const { wrapper, mock, first } = await setup();
    const frame = () => wrapper.get(`[data-testid="board-frame-${first.id}"]`).attributes('style');
    const label = () => wrapper.get(`[data-testid="board-strip-${first.id}"]`).attributes('style');
    const older = structuredClone(mock.state.layout.modes!.board!);
    older.items[0]!.label.x = 11;
    const newer = structuredClone(mock.state.layout.modes!.board!);
    newer.items[0]!.label = { x: 37, y: 5, width: 100, height: 20 };
    newer.items[0]!.rect = { x: 37, y: 25, width: 100, height: 222 };
    mock.emit({ type: 'canvasLive', board: older, seq: 4 });
    mock.emit({ type: 'canvasLive', board: newer, seq: 5 });
    expect(sentOf(mock, 'canvasPainted'), 'nothing before it is drawn').toEqual([]);
    await settle();
    await settle();
    expect(label()).toContain('left: 37px');
    expect(frame()).toContain('height: 222px');
    expect(sentOf(mock, 'canvasPainted')).toEqual([{ type: 'canvasPainted', seq: 5 }]);
    // A state in between draws what it says and answers nothing.
    mock.emit({ type: 'state', ...structuredClone(mock.state) });
    await settle();
    await settle();
    expect(frame()).toContain('height: 400px');
    expect(sentOf(mock, 'canvasPainted')).toHaveLength(1);
  });

  // A turn that came back to where it began still ends: the core puts the views back.
  it('ends a wheel turn that came back to where it began', async () => {
    const { wrapper, mock } = await setup();
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
    try {
      const area = wrapper.get('[data-testid="board"]').element;
      area.dispatchEvent(wheelEvent(-100, { ctrlKey: true, clientX: 30, clientY: 40 }));
      await vi.advanceTimersByTimeAsync(20);
      area.dispatchEvent(wheelEvent(100, { ctrlKey: true, clientX: 30, clientY: 40 }));
      await vi.advanceTimersByTimeAsync(200);
      expect(sentOf(mock, 'canvasZoom')).toEqual([
        { type: 'canvasZoom', x: 30, y: 40, delta: -100, phase: 'move' },
        { type: 'canvasZoom', x: 30, y: 40, delta: 0, phase: 'move' },
        { type: 'canvasZoom', x: 30, y: 40, delta: 0, phase: 'end' },
      ]);
    } finally {
      vi.useRealTimers();
    }
  });

  it('zooms nothing with the wheel zoom switched off, and the page still does not zoom', async () => {
    const { wrapper, mock } = await setup(true, { wheelZoom: false });
    const event = wheelEvent(-100, { ctrlKey: true });
    wrapper.get('[data-testid="board"]').element.dispatchEvent(event);
    await settle();
    expect(event.defaultPrevented).toBe(true);
    expect(sentOf(mock, 'canvasZoom')).toEqual([]);
  });

  it('draws no badge when ui.engineBadge is off', async () => {
    const { wrapper, first } = await setup(true, { settings: { 'ui.engineBadge': false } });
    expect(wrapper.find(`[data-testid="board-engine-${first.id}"]`).exists()).toBe(false);
    expect(wrapper.get(`[data-testid="board-label-${first.id}"]`).text()).toBe(`${first.label}  400 × 800`);
  });
});
