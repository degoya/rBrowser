// The TypeScript half of the Rust <-> TS contract test (RB-020-01). `protocol-fixtures.json`
// holds every message kind in the exact JSON the Rust side (`crates/rb-sync`) writes; here the
// same literals are typed against `protocol.ts`, so `npm run build` (tsc) fails when the TS
// mirror drifts, and the test fails when the file and the typed copy differ. The Rust half
// (`crates/rb-sync/tests/protocol_fixtures.rs`) parses every entry and writes it back unchanged.

import { describe, expect, it } from 'vitest';
import fixtures from './protocol-fixtures.json';
import type { AgentCommand, AgentMessage, CommandResult, ElementTarget } from './protocol';

const at = (path: string): ElementTarget => ({
  element: { kind: 'css', path },
  offset: { x: 0.5, y: 0.5 },
});
const save: ElementTarget = {
  element: { kind: 'testid', testid: 'save', path: '#form > button' },
  offset: { x: 0.25, y: 0.5 },
};

const agentToCore: AgentMessage[] = [
  { kind: 'ready', v: 1, top: true, url: 'http://localhost:5173/' },
  {
    kind: 'event', v: 1, top: false, frame: ['#checkout-frame'], seq: 1, time: 1762790400000,
    event: {
      type: 'pointer', phase: 'down', button: 'left', buttons: 1, pointerType: 'mouse',
      modifiers: 0, target: save,
    },
  },
  {
    kind: 'event', v: 1, top: true, seq: 2, time: 1762790400001,
    event: {
      type: 'click', button: 'left', detail: 1, modifiers: 8,
      target: {
        hosts: [{ kind: 'css', path: 'app-shell' }],
        element: {
          kind: 'role', role: 'button', name: 'Save', nth: 1,
          path: 'main > button:nth-of-type(2)',
        },
        offset: { x: 0.5, y: 0.5 },
      },
    },
  },
  {
    kind: 'event', v: 1, top: true, seq: 3, time: 1762790400002,
    event: {
      type: 'dblclick', button: 'left', detail: 2, modifiers: 0,
      target: {
        element: { kind: 'text', tag: 'span', text: 'Read more', path: 'p > span' },
        offset: { x: 0.1, y: 0.9 },
      },
    },
  },
  {
    kind: 'event', v: 1, top: true, seq: 4, time: 1762790400003,
    event: {
      type: 'contextmenu', button: 'right', detail: 1, modifiers: 2,
      target: at('#list > li:nth-of-type(3)'),
    },
  },
  {
    kind: 'event', v: 1, top: true, seq: 5, time: 1762790400004,
    event: { type: 'hover', target: at('nav > a'), menu: true },
  },
  {
    kind: 'event', v: 1, top: true, seq: 6, time: 1762790400005,
    event: {
      type: 'wheel', deltaX: 0.5, deltaY: 120.5, deltaMode: 0, modifiers: 0, target: at('main'),
    },
  },
  {
    kind: 'event', v: 1, top: true, seq: 7, time: 1762790400006,
    event: {
      type: 'scroll', x: 0.5, y: 640.5, maxX: 0.5, maxY: 2400.5,
      anchor: { element: { kind: 'css', path: '#s5 > h2' }, offset: { x: -0.25, y: 0.125 } },
    },
  },
  {
    kind: 'event', v: 1, top: true, seq: 8, time: 1762790400007,
    event: {
      type: 'scroll', x: 0.5, y: 10.5, maxX: 0.5, maxY: 300.5, container: at('#sidebar'),
    },
  },
  {
    kind: 'event', v: 1, top: true, seq: 9, time: 1762790400008,
    event: {
      type: 'keydown', key: 'Enter', code: 'Enter', keyCode: 13, location: 0, repeat: false,
      modifiers: 0, target: at('#q'),
    },
  },
  {
    kind: 'event', v: 1, top: true, seq: 10, time: 1762790400009,
    event: {
      type: 'keyup', key: 'Escape', code: 'Escape', keyCode: 27, location: 0, repeat: false,
      modifiers: 0,
    },
  },
  {
    kind: 'event', v: 1, top: true, seq: 17, time: 1762790400016,
    event: {
      type: 'keydown', key: 'x', code: 'KeyX', keyCode: 88, location: 0, repeat: false,
      modifiers: 0, sensitive: true, target: at('#password'),
    },
  },
  {
    kind: 'event', v: 1, top: true, seq: 11, time: 1762790400010,
    event: { type: 'input', inputType: 'insertText', data: 'a', value: 'a', target: at('#q') },
  },
  {
    kind: 'event', v: 1, top: true, seq: 12, time: 1762790400011,
    event: {
      type: 'input', inputType: 'insertText', data: 'x', value: 'secretx', sensitive: true,
      target: at('#password'),
    },
  },
  {
    kind: 'event', v: 1, top: true, seq: 13, time: 1762790400012,
    event: {
      type: 'change', value: 'de', selected: [1],
      target: {
        element: { kind: 'role', role: 'combobox', name: 'Language', path: '#lang' },
        offset: { x: 0.5, y: 0.5 },
      },
    },
  },
  {
    kind: 'event', v: 1, top: true, seq: 14, time: 1762790400013,
    event: {
      type: 'change', value: 'on', checked: true,
      target: {
        element: { kind: 'role', role: 'checkbox', name: 'Remember me', path: '#remember' },
        offset: { x: 0.5, y: 0.5 },
      },
    },
  },
  {
    kind: 'event', v: 1, top: true, seq: 15, time: 1762790400014,
    event: {
      type: 'submit', method: 'post', action: 'http://localhost:5173/login', target: at('#login'),
    },
  },
  {
    kind: 'event', v: 1, top: true, seq: 16, time: 1762790400015,
    event: { type: 'focus', target: at('#q') },
  },
  {
    kind: 'bus', v: 1, top: true, time: 1762790400016, origin: 'http://localhost:5173',
    type: 'canvas:pan', payload: { x: 1, y: -2.5, tags: ['a', null, true] },
  },
  { kind: 'detected', v: 1, top: true, tool: 'browsersync' },
  { kind: 'framework', v: 1, top: true, vue: { version: '3.5.22', hook: true, devtools: true } },
  { kind: 'framework', v: 1, top: true, react: { hook: false, devtools: false } },
  { kind: 'framework', v: 1, top: true },
  {
    kind: 'vitals', v: 1, top: true, segment: 0, soft: false, url: 'http://localhost:5173/',
    lcp: { value: 1840, element: '#hero > img' },
    cls: { value: 0.1834, sources: ['#banner', 'main > p:nth-of-type(2)'] },
    inp: { value: 312, target: '#buy', interactions: 4 },
    fcp: 412, ttfb: 96, unsupported: [],
  },
  {
    kind: 'vitals', v: 1, top: true, segment: 2, soft: true, url: 'http://localhost:5173/cart',
    unsupported: ['cls', 'inp'],
  },
];

const coreToAgent: AgentCommand[] = [
  {
    cmd: 'configure',
    config: {
      active: true,
      capture: {
        click: true, hover: false, scroll: true, keyboard: true, input: true, forms: true,
        focus: false,
      },
      hoverThrottleMs: 100,
      scrollDebounceMs: 150,
      scrollMode: 'anchor',
    },
  },
  { cmd: 'replay', ms: 500 },
  { cmd: 'resolve', target: save },
  {
    cmd: 'apply', value: 'de', selected: [1],
    target: {
      element: { kind: 'role', role: 'combobox', name: 'Language', path: '#lang' },
      offset: { x: 0.5, y: 0.5 },
    },
  },
  { cmd: 'apply', value: 'secret', sensitive: true, target: at('#password') },
  { cmd: 'apply', value: 'on', checked: true, target: at('#remember') },
  { cmd: 'focus', target: at('#q') },
  { cmd: 'ping' },
  {
    cmd: 'scroll', x: 0.5, y: 640.5, maxX: 0.5, maxY: 2400.5, container: at('#sidebar'),
    anchor: {
      element: { kind: 'testid', testid: 'row', nth: 2, path: '#sidebar > li:nth-of-type(3)' },
      offset: { x: 0.5, y: 0.25 },
    },
  },
  { cmd: 'point', target: at('main') },
  {
    cmd: 'page',
    page: {
      info: {
        id: 2, label: 'Tablet', width: 768, height: 1024, index: 1, engine: 'chromium',
        emulation: {
          dpr: 2, mobile: true, touch: true, orientation: 'portrait', colorScheme: 'dark',
          reducedMotion: true, locale: 'de-DE',
        },
      },
      bus: { enabled: true, maxPayloadBytes: 65536, maxPerSecond: 100 },
    },
  },
  {
    cmd: 'page',
    page: {
      info: {
        id: 3, label: '500x800', width: 500, height: 800, index: 0, engine: 'chromium',
        emulation: { dpr: 0, mobile: false, touch: false },
      },
      bus: { enabled: false, maxPayloadBytes: 1024, maxPerSecond: 1 },
    },
  },
  {
    cmd: 'bus', from: 1, origin: 'http://localhost:5173', type: 'canvas:pan',
    payload: { x: 1, y: -2.5 },
  },
  { cmd: 'bus', type: 'flow:step', payload: null },
  { cmd: 'navigate', url: 'http://localhost:5173/about' },
];

const results: CommandResult[] = [
  { status: 'done' },
  { status: 'ok', x: 120.5, y: 48.25, scrolled: true },
  { status: 'not_found' },
  { status: 'not_visible' },
  { status: 'error', message: 'unknown command' },
];

describe('sync protocol fixtures', () => {
  it('match the typed agent messages', () => {
    expect(fixtures.agentToCore).toEqual(agentToCore);
  });

  it('match the typed core commands', () => {
    expect(fixtures.coreToAgent).toEqual(coreToAgent);
  });

  it('match the typed command results', () => {
    expect(fixtures.results).toEqual(results);
  });

  it('cover every event type once at least', () => {
    const types = new Set(
      agentToCore.flatMap((message) => (message.kind === 'event' ? [message.event.type] : [])),
    );
    expect([...types].sort()).toEqual(
      [
        'change', 'click', 'contextmenu', 'dblclick', 'focus', 'hover', 'input', 'keydown',
        'keyup', 'pointer', 'scroll', 'submit', 'wheel',
      ].sort(),
    );
  });
});
