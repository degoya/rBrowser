// Applying a change in a follower view (RB-020-05): the state is set the way a framework notices
// - the prototype's value setter past React's tracker, then `input` and `change`; a checkbox or
// radio by a click when its state differs, so it toggles exactly once - and nothing happens when
// the view is already in that state (a replayed click or label click got there first).

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import formsPage from '../../../tests/fixtures/site/forms.html?raw';
import { Agent } from '../agent';
import type { ChangeData, CommandResult, ElementTarget } from '../protocol';
import { el, giveBoxes, loadPage, setViewport } from '../test-page';
import { applyChange, applyFocus } from './apply';

const at = (path: string): ElementTarget => ({
  element: { kind: 'css', path },
  offset: { x: 0.5, y: 0.5 },
});

function apply(data: Omit<ChangeData, 'target'>, path: string): CommandResult {
  return applyChange({ ...data, target: at(path) }, document);
}

/** The events of `types` that reach `element`, in order. */
function listen(element: Element, types: string[]): string[] {
  const seen: string[] = [];
  for (const type of types) element.addEventListener(type, () => seen.push(type));
  return seen;
}

/** React's value tracker (react-dom inputValueTracking): onChange only when the real value
 *  differs from the one a script last set through the element's own property. */
function reactOnChange(element: HTMLInputElement | HTMLSelectElement, field: 'value' | 'checked'): string[] {
  const calls: string[] = [];
  const descriptor = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(element), field)!;
  let current = String((element as unknown as Record<string, unknown>)[field]);
  Object.defineProperty(element, field, {
    configurable: true,
    get() { return descriptor.get!.call(this); },
    set(value: unknown) { current = String(value); descriptor.set!.call(this, value); },
  });
  const types = field === 'checked' ? ['click'] : ['input', 'change'];
  for (const type of types) {
    element.addEventListener(type, () => {
      const next = String(descriptor.get!.call(element));
      if (next !== current) {
        current = next;
        calls.push(next);
      }
    });
  }
  return calls;
}

beforeEach(() => {
  setViewport(1440);
  loadPage(formsPage);
  giveBoxes();
});

afterEach(() => vi.restoreAllMocks());

describe('select', () => {
  it('by index, with input and change; React and Vue see it', () => {
    const size = el<HTMLSelectElement>('#react-size');
    const react = reactOnChange(size, 'value');
    const vue = listen(el('#vue-size'), ['change']);
    expect(apply({ value: 'm', selected: [1] }, '#react-size')).toEqual({ status: 'done' });
    expect(apply({ value: 'm', selected: [1] }, '#vue-size')).toEqual({ status: 'done' });
    expect(size.value).toBe('m');
    expect(react).toEqual(['m']);
    expect(vue).toEqual(['change']);
  });

  it('multiple: exactly the selected indexes', () => {
    const toppings = el<HTMLSelectElement>('#toppings');
    toppings.options[1]!.selected = true;
    const seen = listen(toppings, ['input', 'change']);
    apply({ selected: [0, 3] }, '#toppings');
    expect(Array.from(toppings.selectedOptions, (option) => option.index)).toEqual([0, 3]);
    expect(seen).toEqual(['input', 'change']);
  });

  it('by value when no index is given', () => {
    apply({ value: 'l' }, '#size');
    expect(el<HTMLSelectElement>('#size').value).toBe('l');
  });

  it('nothing when it already shows that state', () => {
    const seen = listen(el('#size'), ['input', 'change']);
    apply({ value: 's', selected: [0] }, '#size');
    expect(seen).toEqual([]);
  });

  it('an option the view does not have: not found', () => {
    expect(apply({ value: 'xl', selected: [7] }, '#size')).toEqual({ status: 'not_found' });
  });
});

describe('checkbox and radio', () => {
  it('a checkbox toggles once by a click; React (click) and Vue (change) see it', () => {
    const agree = el<HTMLInputElement>('#react-agree');
    const react = reactOnChange(agree, 'checked');
    const vue = listen(el('#vue-agree'), ['change']);
    apply({ value: 'on', checked: true }, '#react-agree');
    apply({ value: 'on', checked: true }, '#vue-agree');
    expect(agree.checked).toBe(true);
    expect(el<HTMLInputElement>('#vue-agree').checked).toBe(true);
    expect(react).toEqual(['true']);
    expect(vue).toEqual(['change']);
  });

  it('after a replayed label click the change does not toggle again', () => {
    const newsletter = el<HTMLInputElement>('#newsletter');
    el('#newsletter-label').click();
    expect(newsletter.checked).toBe(true);
    const seen = listen(newsletter, ['click', 'change']);
    apply({ value: 'on', checked: true }, '#newsletter');
    expect(newsletter.checked).toBe(true);
    expect(seen).toEqual([]);
  });

  it('a hidden checkbox (visually hidden, its label shown) is set all the same', () => {
    expect(apply({ value: 'on', checked: true }, '#newsletter')).toEqual({ status: 'done' });
    expect(el<HTMLInputElement>('#newsletter').checked).toBe(true);
  });

  it('a radio group: the other radio goes off', () => {
    apply({ value: 'express', checked: true }, '#ship-exp');
    expect(el<HTMLInputElement>('#ship-exp').checked).toBe(true);
    expect(el<HTMLInputElement>('#ship-std').checked).toBe(false);
  });

  it('a disabled control is left alone', () => {
    const terms = el<HTMLInputElement>('#terms');
    terms.disabled = true;
    apply({ value: 'on', checked: true }, '#terms');
    expect(terms.checked).toBe(false);
  });
});

describe('value controls', () => {
  it('range, colour, date and text: the value, then input and change', () => {
    const cases: [string, string][] = [
      ['#volume', '80'], ['#accent', '#ff0000'], ['#day', '2026-11-10'], ['#city', 'Berlin'],
    ];
    for (const [path, value] of cases) {
      const input = el<HTMLInputElement>(path);
      const seen = listen(input, ['input', 'change']);
      expect(apply({ value }, path)).toEqual({ status: 'done' });
      expect(input.value).toBe(value);
      expect(seen).toEqual(['input', 'change']);
    }
  });

  it('past the React value tracker: onChange fires', () => {
    const name = el<HTMLInputElement>('#react-name');
    const react = reactOnChange(name, 'value');
    apply({ value: 'Ada' }, '#react-name');
    expect(react).toEqual(['Ada']);
  });

  it('Vue sees a range through input', () => {
    const seen = listen(el('#vue-level'), ['input']);
    apply({ value: '7' }, '#vue-level');
    expect(seen).toEqual(['input']);
  });

  it('a password is set; the answer never holds it', () => {
    const result = apply({ value: 'hunter2', sensitive: true }, '#secret');
    expect(el<HTMLInputElement>('#secret').value).toBe('hunter2');
    expect(JSON.stringify(result)).not.toContain('hunter2');
  });

  it('nothing when the value is already there (typed text replayed first)', () => {
    const city = el<HTMLInputElement>('#city');
    city.value = 'Berlin';
    const seen = listen(city, ['input', 'change']);
    apply({ value: 'Berlin' }, '#city');
    expect(seen).toEqual([]);
  });

  it('a file input is never touched', () => {
    const seen = listen(el('#avatar'), ['input', 'change']);
    expect(apply({ value: 'C:\\fakepath\\x.png' }, '#avatar')).toEqual({ status: 'done' });
    expect(seen).toEqual([]);
  });
});

describe('contenteditable', () => {
  it('plain text: set, with an input event', () => {
    const seen = listen(el('#bio'), ['input']);
    apply({ value: 'Hello' }, '#bio');
    expect(el('#bio').textContent).toBe('Hello');
    expect(seen).toEqual(['input']);
  });

  it('rich text is left to the typed input', () => {
    apply({ value: 'flat' }, '#rich');
    expect(el('#rich').innerHTML).toBe('<b>bold</b> text');
  });
});

describe('misses', () => {
  it('an element the view does not have: not found', () => {
    expect(apply({ checked: true }, '#nope')).toEqual({ status: 'not_found' });
  });
});

describe('focus', () => {
  it('focuses the element without scrolling', () => {
    const city = el<HTMLInputElement>('#city');
    const focus = vi.spyOn(city, 'focus');
    expect(applyFocus(at('#city'), document)).toEqual({ status: 'done' });
    expect(focus).toHaveBeenCalledWith({ preventScroll: true });
  });

  it('a missing element: not found', () => {
    expect(applyFocus(at('#nope'), document)).toEqual({ status: 'not_found' });
  });
});

describe('the agent commands', () => {
  let agent: Agent;
  beforeEach(() => {
    agent = new Agent({ win: window, send: () => {}, trusted: () => true });
  });

  it('`apply` and `focus` run', () => {
    expect(agent.command({ cmd: 'apply', value: 'l', selected: [2], target: at('#size') })).toEqual({
      status: 'done',
    });
    expect(el<HTMLSelectElement>('#size').value).toBe('l');
    expect(agent.command({ cmd: 'focus', target: at('#city') })).toEqual({ status: 'done' });
  });

  it('refuse malformed state without quoting it', () => {
    const bad = [
      { cmd: 'apply', target: at('#size'), value: 5 },
      { cmd: 'apply', target: at('#size'), checked: 'yes' },
      { cmd: 'apply', target: at('#size'), selected: [-1] },
      { cmd: 'apply', value: 'secret-value' },
      { cmd: 'focus' },
    ];
    for (const command of bad) {
      const result = agent.command(command);
      expect(result.status).toBe('error');
      expect(JSON.stringify(result)).not.toContain('secret-value');
    }
  });
});
