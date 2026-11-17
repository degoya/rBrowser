// Capture of form controls (RB-020-05): `change` per control type with the state the other views
// need, file inputs without a value, contenteditable as a change on blur, no clicks on controls
// that open a native picker (directly or through their label), and the `forms` / `focus`
// switches.

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import formsPage from '../../../tests/fixtures/site/forms.html?raw';
import { Agent } from '../agent';
import type { AgentConfig, AgentMessage, SyncEvent } from '../protocol';
import { el, giveBoxes, loadPage, setViewport } from '../test-page';

const ACTIVE: AgentConfig = {
  active: true,
  capture: {
    click: true, hover: true, scroll: true, keyboard: true, input: true, forms: true,
    focus: true,
  },
  hoverThrottleMs: 100,
};

let sent: AgentMessage[];
let agent: Agent;

function start(config: AgentConfig = ACTIVE): void {
  agent = new Agent({
    win: window,
    send: (payload) => sent.push(JSON.parse(payload) as AgentMessage),
    config,
    trusted: () => true,
  });
  agent.start();
}

function events(): SyncEvent[] {
  return sent.flatMap((message) => (message.kind === 'event' ? [message.event] : []));
}

const change = () => new Event('change', { bubbles: true });
const click = () => new MouseEvent('click', { bubbles: true, composed: true, detail: 1 });

beforeEach(() => {
  vi.useFakeTimers();
  setViewport(1440);
  loadPage(formsPage);
  giveBoxes();
  sent = [];
});

afterEach(() => {
  agent.stop();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe('change per control type', () => {
  beforeEach(() => start());

  it('a select: its value and the selected index', () => {
    const size = el<HTMLSelectElement>('#size');
    size.selectedIndex = 2;
    size.dispatchEvent(change());
    expect(events()).toMatchObject([{ type: 'change', value: 'l', selected: [2] }]);
  });

  it('a multiple select: every selected index', () => {
    const toppings = el<HTMLSelectElement>('#toppings');
    toppings.options[0]!.selected = true;
    toppings.options[2]!.selected = true;
    toppings.dispatchEvent(change());
    expect(events()).toMatchObject([{ type: 'change', selected: [0, 2] }]);
  });

  it('a radio and a checkbox: value and checked', () => {
    const express = el<HTMLInputElement>('#ship-exp');
    express.checked = true;
    express.dispatchEvent(change());
    const terms = el<HTMLInputElement>('#terms');
    terms.checked = true;
    terms.dispatchEvent(change());
    expect(events()).toMatchObject([
      { type: 'change', value: 'express', checked: true },
      { type: 'change', checked: true },
    ]);
  });

  it('range, color and date: the value', () => {
    for (const [id, value] of [['#volume', '80'], ['#accent', '#ff0000'], ['#day', '2026-11-10']]) {
      const input = el<HTMLInputElement>(id!);
      input.value = value!;
      input.dispatchEvent(change());
    }
    expect(events().map((event) => (event as { value?: string }).value)).toEqual([
      '80', '#ff0000', '2026-11-10',
    ]);
  });

  it('a password: the value, marked sensitive', () => {
    const secret = el<HTMLInputElement>('#secret');
    secret.value = 'hunter2';
    secret.dispatchEvent(change());
    expect(events()).toMatchObject([{ type: 'change', value: 'hunter2', sensitive: true }]);
  });

  it('a file input: no value at all', () => {
    el('#avatar').dispatchEvent(change());
    const [event] = events();
    expect(event).toMatchObject({ type: 'change' });
    expect(Object.keys(event!).sort()).toEqual(['target', 'type']);
  });

  it('a contenteditable: its text as a change when it loses the focus', () => {
    const bio = el('#bio');
    bio.dispatchEvent(new FocusEvent('focusin', { bubbles: true, composed: true }));
    bio.textContent = 'Hello';
    bio.dispatchEvent(new FocusEvent('focusout', { bubbles: true, composed: true }));
    const changes = events().filter((event) => event.type === 'change');
    expect(changes).toMatchObject([{ type: 'change', value: 'Hello' }]);
  });

  it('an unchanged contenteditable: no change', () => {
    const bio = el('#bio');
    bio.dispatchEvent(new FocusEvent('focusin', { bubbles: true, composed: true }));
    bio.dispatchEvent(new FocusEvent('focusout', { bubbles: true, composed: true }));
    expect(events().filter((event) => event.type === 'change')).toEqual([]);
  });
});

describe('clicks on native pickers', () => {
  beforeEach(() => start());

  it('are not captured on a select, a file, a colour or a date input', () => {
    for (const id of ['#size', '#avatar', '#accent', '#day']) el(id).dispatchEvent(click());
    expect(events().filter((event) => event.type === 'click')).toEqual([]);
  });

  it('are not captured on the label of a file input', () => {
    el('#avatar-label').dispatchEvent(click());
    expect(events().filter((event) => event.type === 'click')).toEqual([]);
  });

  it('but a checkbox label is a click like any other', () => {
    el('#newsletter-label').dispatchEvent(click());
    // The label's own click; the engine forwards it to the checkbox (detail 0, not replayed).
    expect(events()[0]).toMatchObject({
      type: 'click', detail: 1, target: { element: { path: '#newsletter-label' } },
    });
  });
});

describe('the switches', () => {
  it('`forms` off: no change', () => {
    start({ ...ACTIVE, capture: { ...ACTIVE.capture, forms: false } });
    el('#terms').dispatchEvent(change());
    expect(events()).toEqual([]);
  });

  it('`focus` off: no focus; on: the focused element', () => {
    start({ ...ACTIVE, capture: { ...ACTIVE.capture, focus: false } });
    el('#city').dispatchEvent(new FocusEvent('focus'));
    expect(events()).toEqual([]);
    agent.command({ cmd: 'configure', config: ACTIVE });
    el('#city').dispatchEvent(new FocusEvent('focus'));
    expect(events()).toMatchObject([{ type: 'focus', target: { element: { role: 'textbox' } } }]);
  });

  it('`input` off leaves change and focus to their own switches', () => {
    start({ ...ACTIVE, capture: { ...ACTIVE.capture, input: false } });
    el('#terms').dispatchEvent(change());
    el('#city').dispatchEvent(new FocusEvent('focus'));
    expect(events().map((event) => event.type)).toEqual(['change', 'focus']);
  });
});
