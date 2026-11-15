import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Agent } from './agent';
import type { AgentConfig, AgentMessage, SyncEvent } from './protocol';
import { el, giveBoxes, loadPage, setViewport } from './test-page';

const ACTIVE: AgentConfig = {
  active: true,
  capture: {
    click: true, hover: true, scroll: true, keyboard: true, input: true, forms: true, focus: true,
  },
  hoverThrottleMs: 100,
};

let sent: AgentMessage[];
let agent: Agent;

function start(config: AgentConfig = ACTIVE, trusted?: (event: Event) => boolean): void {
  agent = new Agent({
    win: window,
    send: (payload) => sent.push(JSON.parse(payload) as AgentMessage),
    config,
    trusted: trusted ?? (() => true),
  });
  agent.start();
}

/** The captured events, without the ready message. */
function events(): SyncEvent[] {
  return sent.flatMap((message) => (message.kind === 'event' ? [message.event] : []));
}

const mouse = { bubbles: true, composed: true, clientX: 35, clientY: 30 };

beforeEach(() => {
  vi.useFakeTimers();
  setViewport(1440);
  loadPage();
  giveBoxes();
  sent = [];
});

afterEach(() => {
  agent.stop();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe('the agent', () => {
  it('says ready with its URL, top frame', () => {
    start();
    expect(sent[0]).toMatchObject({ kind: 'ready', v: 1, top: true, url: window.location.href });
  });

  it('numbers its events and stamps them', () => {
    start();
    const save = el('[data-testid="save"]');
    save.dispatchEvent(new MouseEvent('click', { ...mouse, detail: 1 }));
    save.dispatchEvent(new MouseEvent('click', { ...mouse, detail: 1 }));
    const [first, second] = sent.slice(1);
    expect(first).toMatchObject({ kind: 'event', seq: 1, top: true });
    expect(second).toMatchObject({ kind: 'event', seq: 2 });
    expect(typeof (first as { time: number }).time).toBe('number');
  });
});

describe('capture of every event type', () => {
  beforeEach(() => start());

  it('pointer down and up', () => {
    const save = el('[data-testid="save"]');
    save.dispatchEvent(
      new PointerEvent('pointerdown', { ...mouse, button: 0, buttons: 1, pointerType: 'mouse' }),
    );
    save.dispatchEvent(new PointerEvent('pointerup', { ...mouse, button: 0, pointerType: 'mouse' }));
    expect(events()).toMatchObject([
      {
        type: 'pointer', phase: 'down', button: 'left', buttons: 1, pointerType: 'mouse',
        modifiers: 0,
        target: { element: { kind: 'testid', testid: 'save' }, offset: { x: 0.25, y: 0.25 } },
      },
      { type: 'pointer', phase: 'up', button: 'left' },
    ]);
  });

  it('click, double click and context menu with modifiers', () => {
    const save = el('[data-testid="save"]');
    save.dispatchEvent(new MouseEvent('click', { ...mouse, detail: 1, shiftKey: true }));
    save.dispatchEvent(new MouseEvent('dblclick', { ...mouse, detail: 2 }));
    save.dispatchEvent(new MouseEvent('contextmenu', { ...mouse, button: 2, ctrlKey: true }));
    expect(events()).toMatchObject([
      { type: 'click', button: 'left', detail: 1, modifiers: 8 },
      { type: 'dblclick', detail: 2 },
      { type: 'contextmenu', button: 'right', modifiers: 2 },
    ]);
  });

  it('ignores what happens inside a plugin host element (rbrowser-*)', () => {
    const host = document.createElement('rbrowser-overlays');
    document.body.append(host);
    const button = document.createElement('button');
    host.attachShadow({ mode: 'closed' }).append(button);
    button.dispatchEvent(new MouseEvent('click', { ...mouse, detail: 1 }));
    host.dispatchEvent(new MouseEvent('click', { ...mouse, detail: 1 }));
    button.dispatchEvent(new KeyboardEvent('keydown', { key: 'a', bubbles: true, composed: true }));
    expect(events()).toEqual([]);
  });

  it('hover only when the element changes, throttled', () => {
    const save = el('[data-testid="save"]');
    const cancel = el('button.kGfCyH');
    const move = (target: Element) =>
      target.dispatchEvent(new PointerEvent('pointermove', { ...mouse, pointerType: 'mouse' }));
    move(save);
    move(save);
    move(cancel);
    expect(events()).toMatchObject([{ type: 'hover', target: { element: { testid: 'save' } } }]);
    vi.advanceTimersByTime(100);
    expect(events()).toMatchObject([
      { type: 'hover' },
      { type: 'hover', target: { element: { kind: 'role', name: 'Cancel' } } },
    ]);
  });

  it('wheel, deltas summed per frame', () => {
    const main = el('main');
    main.dispatchEvent(new WheelEvent('wheel', { ...mouse, deltaY: 40.5 }));
    main.dispatchEvent(new WheelEvent('wheel', { ...mouse, deltaY: 40 }));
    vi.advanceTimersByTime(20);
    expect(events()).toMatchObject([{ type: 'wheel', deltaX: 0, deltaY: 80.5, deltaMode: 0 }]);
  });

  it('scroll of the document and of an element', () => {
    document.dispatchEvent(new Event('scroll', { bubbles: true }));
    const list = el('ul.list');
    list.scrollTop = 0;
    list.dispatchEvent(new Event('scroll'));
    vi.advanceTimersByTime(20);
    const scrolls = events();
    expect(scrolls).toHaveLength(2);
    expect(scrolls[0]).toMatchObject({ type: 'scroll' });
    expect(scrolls[0]).not.toHaveProperty('container');
    expect(scrolls[1]).toMatchObject({ type: 'scroll', container: { element: { kind: 'css' } } });
  });

  it('key down and up with the focused element', () => {
    const email = el<HTMLInputElement>('#email');
    email.focus();
    sent = [];
    email.dispatchEvent(
      new KeyboardEvent('keydown', { bubbles: true, composed: true, key: 'Enter', code: 'Enter' }),
    );
    email.dispatchEvent(
      new KeyboardEvent('keyup', { bubbles: true, composed: true, key: 'Enter', code: 'Enter' }),
    );
    expect(events()).toMatchObject([
      { type: 'keydown', key: 'Enter', code: 'Enter', repeat: false, target: { element: { role: 'textbox' } } },
      { type: 'keyup', key: 'Enter' },
    ]);
  });

  it('keys in a password field marked sensitive, others not', () => {
    const password = el<HTMLInputElement>('#password');
    password.focus();
    sent = [];
    password.dispatchEvent(
      new KeyboardEvent('keydown', { bubbles: true, composed: true, key: 'x', code: 'KeyX' }),
    );
    const email = el<HTMLInputElement>('#email');
    email.focus();
    email.dispatchEvent(
      new KeyboardEvent('keydown', { bubbles: true, composed: true, key: 'y', code: 'KeyY' }),
    );
    const keys = events().filter((event) => event.type === 'keydown');
    expect(keys).toMatchObject([
      { type: 'keydown', key: 'x', sensitive: true },
      { type: 'keydown', key: 'y' },
    ]);
    expect(keys[1]).not.toHaveProperty('sensitive');
  });

  it('input with its value; password marked sensitive', () => {
    const email = el<HTMLInputElement>('#email');
    email.value = 'a';
    email.dispatchEvent(
      new InputEvent('input', { bubbles: true, composed: true, inputType: 'insertText', data: 'a' }),
    );
    const password = el<HTMLInputElement>('#password');
    password.value = 'x';
    password.dispatchEvent(
      new InputEvent('input', { bubbles: true, composed: true, inputType: 'insertText', data: 'x' }),
    );
    expect(events()).toMatchObject([
      { type: 'input', inputType: 'insertText', data: 'a', value: 'a' },
      { type: 'input', value: 'x', sensitive: true },
    ]);
    expect(events()[0]).not.toHaveProperty('sensitive');
  });

  it('change of a checkbox and a select', () => {
    const remember = el<HTMLInputElement>('#remember');
    remember.checked = true;
    remember.dispatchEvent(new Event('change', { bubbles: true }));
    const lang = el<HTMLSelectElement>('#lang');
    lang.selectedIndex = 1;
    lang.dispatchEvent(new Event('change', { bubbles: true }));
    expect(events()).toMatchObject([
      { type: 'change', checked: true, target: { element: { role: 'checkbox' } } },
      { type: 'change', value: 'de', selected: [1] },
    ]);
  });

  it('submit with method and action', () => {
    el('#login').dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    expect(events()).toMatchObject([
      { type: 'submit', method: 'post', target: { element: { kind: 'css', path: '#login' } } },
    ]);
    expect((events()[0] as { action: string }).action).toMatch(/\/login$/);
  });

  it('focus', () => {
    el('#email').dispatchEvent(new FocusEvent('focus'));
    expect(events()).toMatchObject([{ type: 'focus', target: { element: { role: 'textbox' } } }]);
  });
});

describe('what is not captured', () => {
  it('nothing while the view is not active', () => {
    start({ ...ACTIVE, active: false });
    el('[data-testid="save"]').dispatchEvent(new MouseEvent('click', mouse));
    expect(events()).toEqual([]);
  });

  it('nothing of a switched-off kind', () => {
    start({ ...ACTIVE, capture: { ...ACTIVE.capture, click: false } });
    el('[data-testid="save"]').dispatchEvent(new MouseEvent('click', mouse));
    expect(events()).toEqual([]);
  });

  it('no event a page script made (untrusted)', () => {
    start(ACTIVE, (event) => event.isTrusted);
    el('[data-testid="save"]').dispatchEvent(new MouseEvent('click', mouse));
    expect(events()).toEqual([]);
  });

  it('no echo: replayed events are not captured again', () => {
    start();
    expect(agent.command({ cmd: 'replay', ms: 500 })).toEqual({ status: 'done' });
    const save = el('[data-testid="save"]');
    save.dispatchEvent(new MouseEvent('click', mouse));
    expect(events()).toEqual([]);
    expect(agent.command({ cmd: 'replay', ms: 0 })).toEqual({ status: 'done' });
    save.dispatchEvent(new MouseEvent('click', mouse));
    expect(events()).toHaveLength(1);
  });

  it('the replay marker ends by itself', () => {
    start();
    agent.command({ cmd: 'replay', ms: 200 });
    vi.advanceTimersByTime(201);
    el('[data-testid="save"]').dispatchEvent(new MouseEvent('click', mouse));
    expect(events()).toHaveLength(1);
  });

  it('nothing after stop', () => {
    start();
    agent.stop();
    el('[data-testid="save"]').dispatchEvent(new MouseEvent('click', mouse));
    expect(events()).toEqual([]);
  });
});

describe('the capture hot path (RB-080-27)', () => {
  beforeEach(() => start());

  /** How often the agent itself reads the composed path for `dispatch`: the calls with the
   *  agent capturing, less those with the same listeners installed but every event untrusted
   *  (happy-dom reads the path itself while it dispatches). */
  function pathReads(dispatch: () => void): number {
    const path = vi.spyOn(Event.prototype, 'composedPath');
    path.mockClear();
    agent.stop();
    start(ACTIVE, () => false);
    dispatch();
    const dom = path.mock.calls.length;
    path.mockClear();
    agent.stop();
    start();
    dispatch();
    return path.mock.calls.length - dom;
  }

  it('reads the composed path once per event', () => {
    const save = el('[data-testid="save"]');
    const email = el<HTMLInputElement>('#email');
    expect(
      pathReads(() => save.dispatchEvent(new MouseEvent('click', { ...mouse, detail: 1 }))),
    ).toBe(1);
    expect(
      pathReads(() =>
        save.dispatchEvent(new PointerEvent('pointermove', { ...mouse, pointerType: 'mouse' })),
      ),
    ).toBe(1);
    email.focus();
    expect(
      pathReads(() =>
        email.dispatchEvent(new KeyboardEvent('keydown', { key: 'a', bubbles: true, composed: true })),
      ),
    ).toBe(1);
    const captured = events().map((event) => event.type);
    expect(captured.filter((type) => type !== 'focus')).toEqual(['click', 'hover', 'keydown']);
  });

  it('walks no ancestors for a pointermove over the same element', () => {
    const save = el('[data-testid="save"]');
    const cancel = el('button.kGfCyH');
    const move = (target: Element) =>
      target.dispatchEvent(new PointerEvent('pointermove', { ...mouse, pointerType: 'mouse' }));
    const closest = vi.spyOn(Element.prototype, 'closest');
    move(save);
    const walked = closest.mock.calls.length;
    expect(walked).toBeGreaterThan(0);
    move(save);
    move(save);
    expect(closest).toHaveBeenCalledTimes(walked);
    move(cancel);
    expect(closest.mock.calls.length).toBeGreaterThan(walked);
    vi.advanceTimersByTime(100);
    expect(events().map((event) => event.type)).toEqual(['hover', 'hover']);
  });
});
