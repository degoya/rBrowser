// The page clock and the animation step (RB-094-07, `crates/rbrowser/src/time_control/clock.js`
// and `step.js`): rBrowser registers the clock in every view's main world with its
// configuration (`script.rs`: the script, then `set(config)`), and calls the step in its
// isolated world. Here they run in happy-dom's window with Node's `Date` and `Intl`; the real
// time is `Date.now` of the native `Date`, mocked where a test needs it to move. The fixture
// page `tests/fixtures/site/clock.html` is the acceptance's: with the clock at 2026-12-24 10:00
// it reports that date. happy-dom has no Web Animations, so the rate itself (the timeline over
// CDP) is the e2e run's (`npm run test:clock`).
import { afterEach, describe, expect, it, vi } from 'vitest';
import clockSource from '../../crates/rbrowser/src/time_control/clock.js?raw';
import stepSource from '../../crates/rbrowser/src/time_control/step.js?raw';
import fixture from '../../tests/fixtures/site/clock.html?raw';

interface ClockConfig {
  origin?: number;
  local?: number[];
  offset?: number;
  anchor: number;
  speed: number;
}

interface ClockApi {
  set(config: unknown): boolean;
  reset(): boolean;
  now(): number;
}

interface FixtureClock {
  local: string;
  epochMs: number;
  intl: string;
  daysToNewYear: number;
}

const NativeDate = globalThis.Date;
const KEY = Symbol.for('rbrowser.clock');
const FORMAT = Object.getOwnPropertyDescriptor(Intl.DateTimeFormat.prototype, 'format');
const FORMAT_TO_PARTS = Intl.DateTimeFormat.prototype.formatToParts;

function api(): ClockApi {
  const scope = globalThis as unknown as Record<symbol, ClockApi | undefined>;
  if (!scope[KEY]) {
    new Function(clockSource)();
  }
  const found = scope[KEY];
  if (!found) throw new Error('the script defined no clock');
  return found;
}

/** What rBrowser registers in a view: the script, then its configuration as JSON. */
function install(config: ClockConfig): void {
  new Function(`${clockSource}\n;globalThis[Symbol.for("rbrowser.clock")].set(${JSON.stringify(config)});`)();
}

const pad = (value: number): string => String(value).padStart(2, '0');

/** `YYYY-MM-DD HH:MM` of `date` in local time, as the fixture writes it. */
function localText(date: Date): string {
  return (
    `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ` +
    `${pad(date.getHours())}:${pad(date.getMinutes())}`
  );
}

afterEach(() => {
  api().reset();
  vi.restoreAllMocks();
});

describe('page clock', () => {
  it('reads a fixed local time in Date and Intl, and holds it while the real time runs', () => {
    const real = vi.spyOn(NativeDate, 'now').mockReturnValue(1_700_000_000_000);
    install({ local: [2026, 12, 24, 10, 0, 0, 0], anchor: 1_700_000_000_000, speed: 0 });
    const expected = new NativeDate(2026, 11, 24, 10, 0, 0, 0).getTime();
    expect(Date.now()).toBe(expected);
    const now = new Date();
    expect([now.getFullYear(), now.getMonth(), now.getDate(), now.getHours(), now.getMinutes()]).toEqual([
      2026, 11, 24, 10, 0,
    ]);
    expect(now.toLocaleDateString('en-US')).toBe('12/24/2026');
    expect(new Intl.DateTimeFormat('en-US', { year: 'numeric', month: '2-digit', day: '2-digit' }).format()).toBe(
      '12/24/2026',
    );
    const year = new Intl.DateTimeFormat('en-US', { year: 'numeric' }).formatToParts().find((part) => part.type === 'year');
    expect(year?.value).toBe('2026');
    expect(Date()).toContain('2026');
    // The real time runs on; the fixed clock does not.
    real.mockReturnValue(1_700_000_060_000);
    expect(Date.now()).toBe(expected);
  });

  it('leaves dates with arguments, the static functions and subclasses as they are', () => {
    install({ origin: NativeDate.UTC(2026, 11, 24, 9), anchor: NativeDate.now(), speed: 0 });
    expect(new Date(0).getTime()).toBe(0);
    expect(new Date(2020, 0, 1).getFullYear()).toBe(2020);
    expect(Date.UTC(2020, 0, 1)).toBe(NativeDate.UTC(2020, 0, 1));
    expect(Date.parse('2020-01-01T00:00:00Z')).toBe(NativeDate.parse('2020-01-01T00:00:00Z'));
    const now = new Date();
    expect(now instanceof Date).toBe(true);
    expect(now instanceof NativeDate).toBe(true);
    expect(Date.prototype.constructor).toBe(Date);
    expect(Date.name).toBe('Date');
    class Stamp extends Date {}
    const stamp = new Stamp();
    expect(stamp instanceof Stamp).toBe(true);
    expect(stamp.getTime()).toBe(NativeDate.UTC(2026, 11, 24, 9));
    const format = new Intl.DateTimeFormat('en-US', { timeZone: 'UTC', year: 'numeric', month: '2-digit', day: '2-digit' });
    expect(format.format(new NativeDate(NativeDate.UTC(2020, 0, 1, 12)))).toBe('01/01/2020');
    expect(format.format).toBe(format.format);
  });

  it('runs from the anchor at its speed and keeps what ran before a change', () => {
    const anchor = 1_800_000_000_000;
    const origin = NativeDate.UTC(2026, 11, 24, 9);
    const real = vi.spyOn(NativeDate, 'now').mockReturnValue(anchor);
    install({ origin, anchor, speed: 2 });
    expect(Date.now()).toBe(origin);
    real.mockReturnValue(anchor + 1500);
    expect(Date.now()).toBe(origin + 3000);
    expect(new Date().getTime()).toBe(origin + 3000);
    // A new speed from the real time on, with the 3 s that ran as offset (`Clock::with_speed`).
    install({ origin, offset: 3000, anchor: anchor + 1500, speed: 1 });
    real.mockReturnValue(anchor + 2500);
    expect(Date.now()).toBe(origin + 4000);
  });

  it('takes a second configuration and gives the real time back on reset', () => {
    const real = vi.spyOn(NativeDate, 'now').mockReturnValue(1_750_000_000_000);
    install({ origin: 0, anchor: 1_750_000_000_000, speed: 0 });
    install({ origin: 86_400_000, anchor: 1_750_000_000_000, speed: 0 });
    expect(Date.now()).toBe(86_400_000);
    expect(api().reset()).toBe(true);
    expect(globalThis.Date).toBe(NativeDate);
    expect(Date.prototype.constructor).toBe(NativeDate);
    expect(Date.now()).toBe(1_750_000_000_000);
    expect(Object.getOwnPropertyDescriptor(Intl.DateTimeFormat.prototype, 'format')).toEqual(FORMAT);
    expect(Intl.DateTimeFormat.prototype.formatToParts).toBe(FORMAT_TO_PARTS);
    real.mockRestore();
  });

  it('refuses a configuration that is no clock and changes nothing', () => {
    expect(api().set(null)).toBe(false);
    expect(api().set({ origin: 1, anchor: Number.NaN, speed: 0 })).toBe(false);
    expect(api().set({ origin: 1, anchor: 1, speed: -1 })).toBe(false);
    expect(api().set({ local: [2026, 12], anchor: 1, speed: 0 })).toBe(false);
    expect(globalThis.Date).toBe(NativeDate);
  });
});

describe('the clock fixture', () => {
  /** The fixture's own script, run in this window; its `readClock`. */
  function loadFixture(): () => FixtureClock {
    const body = /<body>([\s\S]*?)<script>/.exec(fixture)?.[1] ?? '';
    const script = /<script>([\s\S]*?)<\/script>/.exec(fixture)?.[1] ?? '';
    document.body.innerHTML = body;
    // No timer of its own: the test reads when it wants.
    return new Function('setInterval', `${script}\nreturn readClock;`)(() => 0) as () => FixtureClock;
  }

  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('reports 2026-12-24 10:00 with the clock there, and the real time after the reset', () => {
    const readClock = loadFixture();
    install({ local: [2026, 12, 24, 10, 0, 0, 0], anchor: NativeDate.now(), speed: 0 });
    const fixed = readClock();
    expect(fixed.local).toBe('2026-12-24 10:00');
    expect(fixed.intl).toBe('Dec 24, 2026');
    expect(fixed.daysToNewYear).toBe(8);
    api().reset();
    const before = localText(new NativeDate());
    const real = readClock();
    const after = localText(new NativeDate());
    expect(real.local).not.toBe('2026-12-24 10:00');
    expect([before, after]).toContain(real.local);
  });
});

describe('animation step', () => {
  function withAnimations(animations: { currentTime: unknown }[]): void {
    Object.defineProperty(document, 'getAnimations', { value: () => animations, configurable: true });
  }

  function step(ms: unknown): number {
    return new Function(`return (${stepSource})(${JSON.stringify(ms)});`)() as number;
  }

  afterEach(() => {
    Reflect.deleteProperty(document, 'getAnimations');
  });

  it('moves every animation with a time on by the step', () => {
    const css = { currentTime: 1200 };
    const web = { currentTime: 0 };
    const idle = { currentTime: null };
    const scroll = { currentTime: { value: 40, unit: 'percent' } };
    withAnimations([css, web, idle, scroll]);
    expect(step(100)).toBe(2);
    expect(css.currentTime).toBe(1300);
    expect(web.currentTime).toBe(100);
    expect(idle.currentTime).toBeNull();
    expect(scroll.currentTime).toEqual({ value: 40, unit: 'percent' });
  });

  it('moves nothing for a step that is no time, or without Web Animations', () => {
    const css = { currentTime: 1200 };
    withAnimations([css]);
    for (const ms of [0, -5, 'soon', null]) {
      expect(step(ms)).toBe(0);
    }
    expect(css.currentTime).toBe(1200);
    Reflect.deleteProperty(document, 'getAnimations');
    expect(step(100)).toBe(0);
  });
});
