// RB-092-01: the editor's kinds - grouped, what each needs, the action built from an element and a
// value, and a kind changed keeping what fits.
import { describe, expect, it } from 'vitest';
import type { Locator } from './recording';
import {
  KIND_GROUPS,
  actionFor,
  changeKind,
  editorValue,
  elementOf,
  groupOf,
  kindOf,
  valueOf,
} from './stepKinds';

const button: Locator = { selectors: [{ kind: 'role', role: 'button', name: 'Filter' }, { kind: 'css', path: 'aside > button' }] };

describe('the kinds of step', () => {
  it('come in five groups, each kind once', () => {
    expect(KIND_GROUPS.map((entry) => entry.group)).toEqual(['navigation', 'interaction', 'input', 'wait', 'assert']);
    const kinds = KIND_GROUPS.flatMap((entry) => entry.kinds);
    expect(new Set(kinds).size).toBe(kinds.length);
    expect(kinds).toHaveLength(17);
    expect(groupOf('wait')).toBe('wait');
    expect(groupOf('assertText')).toBe('assert');
  });

  it('say whether they need an element and which value', () => {
    expect(elementOf('click')).toBe('required');
    expect(elementOf('press')).toBe('optional');
    expect(elementOf('navigate')).toBe('none');
    expect(elementOf('assertUrl')).toBe('none');
    expect(valueOf('navigate')).toBe('url');
    expect(valueOf('press')).toBe('key');
    expect(valueOf('wait')).toBe('ms');
    expect(valueOf('select')).toBe('option');
    expect(valueOf('click')).toBeNull();
  });
});

describe('an action from a kind, an element and a value', () => {
  it('is built when nothing is missing', () => {
    expect(actionFor('click', button, '')).toEqual({ type: 'click', target: button });
    expect(actionFor('fill', button, 'running shoes')).toEqual({ type: 'fill', target: button, value: 'running shoes' });
    expect(actionFor('navigate', null, ' https://example.test/ ')).toEqual({ type: 'navigate', url: 'https://example.test/' });
    expect(actionFor('wait', null, '1500')).toEqual({ type: 'wait', ms: 1500 });
    expect(actionFor('press', null, 'Enter')).toEqual({ type: 'press', key: 'Enter' });
    expect(actionFor('assertText', button, 'Filter')).toEqual({
      type: 'assert',
      assertion: { kind: 'text', target: button, text: 'Filter' },
    });
    expect(actionFor('assertUrl', null, 'https://example.test/s')).toEqual({
      type: 'assert',
      assertion: { kind: 'url', url: 'https://example.test/s' },
    });
  });

  it('is none while the element, the URL, the key or a valid time is missing', () => {
    expect(actionFor('click', null, '')).toBeNull();
    expect(actionFor('navigate', null, '  ')).toBeNull();
    expect(actionFor('press', null, '')).toBeNull();
    expect(actionFor('wait', null, '10001')).toBeNull();
    expect(actionFor('wait', null, '1.5')).toBeNull();
    expect(actionFor('assertVisible', null, '')).toBeNull();
  });
});

describe('a step whose kind changes', () => {
  it('keeps its element and a value of the same sort', () => {
    const fill = actionFor('fill', button, 'shoes');
    expect(fill).not.toBeNull();
    if (fill === null) {
      return;
    }
    expect(changeKind(fill, 'hover')).toEqual({ type: 'hover', target: button });
    expect(changeKind(fill, 'assertText')).toEqual({
      type: 'assert',
      assertion: { kind: 'text', target: button, text: 'shoes' },
    });
    expect(changeKind(fill, 'navigate')).toBeNull();
  });

  it('reads back its kind and value', () => {
    expect(kindOf({ type: 'wait', ms: 800 })).toBe('wait');
    expect(editorValue({ type: 'wait', ms: 800 })).toBe('800');
    expect(kindOf({ type: 'assert', assertion: { kind: 'visible', target: button } })).toBe('assertVisible');
    expect(editorValue({ type: 'click', target: button })).toBe('');
  });
});
