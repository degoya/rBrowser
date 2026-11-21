import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Agent } from './agent';
import { installCapture } from './capture';
import type { AgentConfig, AgentMessage, PathPoint, RecordPointerMode, RecordConfig, SyncEvent } from './protocol';

// A navigation with two menus: Products opens a mega menu whose Software entry opens a second
// level; Careers a plain dropdown. Each submenu is hidden until the test shows it, as the page's
// `li:hover > ul` would.
const NAV = `
  <nav>
    <ul id="menu">
      <li><a id="home" href="/">Home</a></li>
      <li id="products">
        <a id="products-link" href="/products">Products</a>
        <div class="mega" id="mega" style="display: none">
          <ul>
            <li id="software">
              <a id="software-link" href="/software">Software</a>
              <ul id="software-menu" style="display: none">
                <li><a id="editor" href="/software/editor">Editor</a></li>
                <li><a id="compiler" href="/software/compiler">Compiler</a></li>
              </ul>
            </li>
            <li><a id="hardware" href="/hardware">Hardware</a></li>
          </ul>
        </div>
      </li>
      <li id="careers">
        <a id="careers-link" href="/careers">Careers</a>
        <ul id="careers-menu" style="display: none">
          <li><a id="trainees" href="/careers/trainees">Trainees</a></li>
        </ul>
      </li>
    </ul>
  </nav>
  <main><a id="plain" href="/plain">Plain link</a></main>`;

const by = (id: string): HTMLElement => {
  const element = document.getElementById(id);
  if (!element) throw new Error(`no #${id}`);
  return element;
};
const show = (id: string) => by(id).style.removeProperty('display');
const hide = (id: string) => by(id).style.setProperty('display', 'none');

interface Emitted {
  event: SyncEvent;
  path?: PathPoint[];
}

function record(recordPointer: RecordPointerMode = 'menu', hoverDwellMs = 250): RecordConfig {
  return { menuHovers: true, hoverDwellMs, recordPointer };
}

function capture(config: RecordConfig | null) {
  const emitted: Emitted[] = [];
  const installed = installCapture(window, {
    enabled: () => true,
    trusted: () => true,
    hoverThrottleMs: () => 0,
    wheel: () => false,
    record: () => config,
    emit: (event, path) => emitted.push(path ? { event, path } : { event }),
  });
  const rests = () =>
    emitted
      .filter(({ event }) => event.type === 'hover' && event.menu === true)
      .map(({ event }) => (event.type === 'hover' && event.target.element.kind === 'role' ? event.target.element.name : '?'));
  return { emitted, installed, rests };
}

/** Moves the pointer onto `id` in steps from (`x`, `y`), `ms` apart. */
function moveTo(id: string, x: number, y: number, ms = 20) {
  for (const dx of [0, 20, 40]) {
    by(id).dispatchEvent(
      new PointerEvent('pointermove', { bubbles: true, composed: true, pointerType: 'mouse', clientX: x + dx, clientY: y }),
    );
    vi.advanceTimersByTime(ms);
  }
}

function press(id: string, type: 'pointerdown' | 'click' = 'pointerdown') {
  const init = { bubbles: true, composed: true, pointerType: 'mouse', button: 0, clientX: 300, clientY: 300 };
  by(id).dispatchEvent(type === 'click' ? new MouseEvent('click', init) : new PointerEvent('pointerdown', init));
}

beforeEach(() => {
  vi.useFakeTimers();
  document.body.innerHTML = NAV;
});

afterEach(() => {
  vi.useRealTimers();
});

describe('a recording keeps every rest on a menu entry', () => {
  it('on both levels of a menu and on another top-level menu, without a click', () => {
    const { installed, rests } = capture(record());
    moveTo('products-link', 10, 10);
    show('mega');
    vi.advanceTimersByTime(250);
    moveTo('software-link', 10, 60);
    show('software-menu');
    vi.advanceTimersByTime(250);
    moveTo('editor', 200, 60);
    vi.advanceTimersByTime(250);
    moveTo('compiler', 200, 90);
    vi.advanceTimersByTime(250);
    hide('mega');
    moveTo('careers-link', 100, 10);
    show('careers-menu');
    vi.advanceTimersByTime(250);
    installed.stop();
    expect(rests()).toEqual(['Products', 'Software', 'Editor', 'Compiler', 'Careers']);
  });

  it('leaves out an entry the pointer passed over quicker than the dwell', () => {
    const { installed, rests } = capture(record());
    show('mega');
    show('software-menu');
    moveTo('products-link', 10, 10);
    moveTo('software-link', 10, 60);
    moveTo('editor', 200, 60);
    vi.advanceTimersByTime(300);
    installed.stop();
    // 60 ms on each of the first two; the pointer rests on the last.
    expect(rests()).toEqual(['Editor']);
  });

  it('leaves out a rest on a link that is no menu entry, and a top-level entry with no submenu', () => {
    const { installed, rests } = capture(record());
    moveTo('plain', 10, 300);
    vi.advanceTimersByTime(500);
    moveTo('home', 10, 10);
    vi.advanceTimersByTime(500);
    installed.stop();
    expect(rests()).toEqual([]);
  });

  it('waits for a submenu that opens after a hover delay', () => {
    const { installed, rests } = capture(record());
    moveTo('careers-link', 100, 10);
    vi.advanceTimersByTime(250);
    expect(rests()).toEqual([]);
    show('careers-menu');
    vi.advanceTimersByTime(100);
    installed.stop();
    expect(rests()).toEqual(['Careers']);
  });

  it('keeps nothing while no recording runs or it records no hover', () => {
    for (const config of [null, { ...record(), menuHovers: false }]) {
      const { installed, rests } = capture(config);
      show('careers-menu');
      moveTo('careers-link', 100, 10);
      vi.advanceTimersByTime(500);
      installed.stop();
      expect(rests()).toEqual([]);
    }
  });

  it('drops the rest still waiting when the button goes down', () => {
    const { installed, rests } = capture(record());
    show('careers-menu');
    moveTo('careers-link', 100, 10);
    press('careers-link');
    vi.advanceTimersByTime(500);
    installed.stop();
    expect(rests()).toEqual([]);
  });

  it('drops the rest still waiting when the pointer leaves the page, the window or the tab', () => {
    const leaves: Array<() => void> = [
      // To rBrowser's toolbar: the pointer leaves the document, no element takes it.
      () =>
        by('careers-link').dispatchEvent(
          new PointerEvent('pointerout', { bubbles: true, composed: true, pointerType: 'mouse', relatedTarget: null }),
        ),
      () => window.dispatchEvent(new FocusEvent('blur')),
      () => document.dispatchEvent(new Event('visibilitychange', { bubbles: true })),
    ];
    for (const leave of leaves) {
      const { installed, rests } = capture(record());
      show('careers-menu');
      moveTo('careers-link', 100, 10);
      leave();
      vi.advanceTimersByTime(1_500);
      installed.stop();
      expect(rests()).toEqual([]);
    }
  });

  it('keeps the rest when the pointer only moves from one element to another', () => {
    const { installed, rests } = capture(record());
    show('careers-menu');
    moveTo('careers-link', 100, 10);
    by('careers-link').dispatchEvent(
      new PointerEvent('pointerout', { bubbles: true, composed: true, pointerType: 'mouse', relatedTarget: by('careers') }),
    );
    // An element's blur is no leaving of the window.
    by('careers-link').dispatchEvent(new FocusEvent('blur'));
    vi.advanceTimersByTime(300);
    installed.stop();
    expect(rests()).toEqual(['Careers']);
  });
});

describe('the menu hover at a press (RB-090-76) while recording', () => {
  it('is not reported again after a rest on that menu', () => {
    const { emitted, installed } = capture(record());
    show('careers-menu');
    moveTo('careers-link', 100, 10);
    vi.advanceTimersByTime(250);
    moveTo('trainees', 100, 40);
    vi.advanceTimersByTime(250);
    press('trainees');
    installed.stop();
    const menu = emitted.filter(({ event }) => event.type === 'hover' && event.menu === true);
    expect(menu).toHaveLength(2);
  });

  it('is reported when the last rest was on another menu', () => {
    const { emitted, installed } = capture(record());
    show('mega');
    moveTo('products-link', 10, 10);
    vi.advanceTimersByTime(250);
    show('careers-menu');
    moveTo('trainees', 100, 40);
    press('trainees');
    installed.stop();
    const menu = emitted.filter(({ event }) => event.type === 'hover' && event.menu === true);
    expect(menu.map(({ event }) => (event.type === 'hover' ? event.target.element.kind : ''))).toHaveLength(2);
    expect(menu[1]?.event).toMatchObject({ target: { element: { name: 'Careers' } } });
  });
});

describe('the pointer path of a recording', () => {
  it('goes with a rest and with the click into the open menu (`menu`)', () => {
    const { emitted, installed } = capture(record('menu'));
    show('careers-menu');
    moveTo('careers-link', 100, 10);
    vi.advanceTimersByTime(250);
    moveTo('trainees', 100, 40);
    press('trainees');
    press('trainees', 'click');
    installed.stop();
    const rest = emitted.find(({ event }) => event.type === 'hover' && event.menu === true);
    expect(rest?.path?.map((point) => point.x)).toEqual([100, 120, 140]);
    expect(rest?.path?.[0]?.t).toBe(0);
    const click = emitted.find(({ event }) => event.type === 'click');
    expect(click?.path?.length).toBeGreaterThan(1);
    expect(click?.path?.at(-1)).toMatchObject({ x: 300, y: 300 });
  });

  it('leaves a plain click without its way at `menu`, and gives it at `all`', () => {
    for (const [mode, expected] of [['menu', false], ['all', true]] as const) {
      const { emitted, installed } = capture(record(mode));
      moveTo('plain', 10, 300);
      press('plain');
      press('plain', 'click');
      installed.stop();
      const click = emitted.find(({ event }) => event.type === 'click');
      expect(click?.path !== undefined).toBe(expected);
    }
  });

  it('keeps no way at `off`', () => {
    const { emitted, installed } = capture(record('off'));
    show('careers-menu');
    moveTo('careers-link', 100, 10);
    vi.advanceTimersByTime(250);
    moveTo('trainees', 100, 40);
    press('trainees');
    press('trainees', 'click');
    installed.stop();
    expect(emitted.some((entry) => entry.path !== undefined)).toBe(false);
    expect(emitted.some(({ event }) => event.type === 'hover' && event.menu === true)).toBe(true);
  });
});

describe('the agent of the recorded view', () => {
  const base: AgentConfig = {
    active: true,
    capture: { click: true, hover: true, scroll: true, keyboard: true, input: true, forms: true, focus: false },
    hoverThrottleMs: 100,
  };

  it('sends the way beside the event, and none without a recording', () => {
    for (const [config, expected] of [[{ ...base, record: record('all') }, true], [base, false]] as const) {
      const sent: AgentMessage[] = [];
      const agent = new Agent({
        win: window,
        send: (payload) => sent.push(JSON.parse(payload) as AgentMessage),
        config,
        trusted: () => true,
      });
      agent.start();
      moveTo('plain', 10, 300);
      press('plain');
      press('plain', 'click');
      agent.stop();
      const click = sent.find((message) => message.kind === 'event' && message.event.type === 'click');
      expect(click !== undefined && 'path' in click).toBe(expected);
    }
  });

  it('refuses a malformed recording configuration', () => {
    const agent = new Agent({ win: window, send: () => {}, config: base });
    for (const bad of [
      { menuHovers: 'yes', hoverDwellMs: 250, recordPointer: 'menu' },
      { menuHovers: true, hoverDwellMs: -1, recordPointer: 'menu' },
      { menuHovers: true, hoverDwellMs: 250, recordPointer: 'some' },
    ]) {
      expect(agent.command({ cmd: 'configure', config: { ...base, record: bad } })).toMatchObject({ status: 'error' });
    }
    expect(agent.command({ cmd: 'configure', config: { ...base, record: record() } })).toEqual({ status: 'done' });
  });
});
