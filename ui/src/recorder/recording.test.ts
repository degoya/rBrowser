import { describe, expect, it } from 'vitest';
import {
  deleteStep,
  moveStep,
  renameStep,
  setStepValue,
  stepKind,
  stepValue,
  targetText,
  type Recording,
  type Step,
} from './recording';

const css = (path: string) => ({ selectors: [{ kind: 'css' as const, path }] });

function sample(): Recording {
  const steps: Step[] = [
    { id: 1, action: { type: 'navigate', url: 'http://shop.test/' }, view: 'desktop', atMs: 0 },
    {
      id: 2,
      name: 'Open the menu',
      action: { type: 'click', target: { selectors: [{ kind: 'testid', testid: 'burger' }, { kind: 'css', path: '#burger' }] } },
      view: 'mobile',
      atMs: 300,
      onlyIn: ['mobile'],
    },
    { id: 3, action: { type: 'fill', target: css('#email'), value: 'ada@shop.test' }, view: 'desktop', atMs: 900 },
    {
      id: 4,
      action: { type: 'fill', target: css('#password'), value: '{{secret:password}}', secret: true },
      view: 'desktop',
      atMs: 1200,
    },
    {
      id: 5,
      action: { type: 'assert', assertion: { kind: 'text', target: css('h1'), text: 'Welcome' } },
      view: 'desktop',
      atMs: 1500,
    },
  ];
  return { version: 1, name: 'login', startedAtMs: 1, appVersion: '0.9.0', breakpoints: [], steps };
}

/** Step `index` of `steps`, which the test knows is there. */
function nth(steps: Step[], index: number): Step {
  const step = steps[index];
  if (step === undefined) {
    throw new Error(`no step ${index}`);
  }
  return step;
}

describe('the recording model of the Recorder panel (RB-090-60)', () => {
  it('names each step kind, assertions and the wait included', () => {
    const steps = sample().steps;
    expect(steps.map((step) => stepKind(step.action))).toEqual(['navigate', 'click', 'fill', 'fill', 'assertText']);
    expect(stepKind({ type: 'waitFor', target: css('#done') })).toBe('waitFor');
    expect(stepKind({ type: 'assert', assertion: { kind: 'visible', target: css('h1') } })).toBe('assertVisible');
  });

  it('shows a target by its best selector', () => {
    const steps = sample().steps;
    expect(targetText(nth(steps, 1).action)).toBe('[data-testid="burger"]');
    expect(targetText(nth(steps, 2).action)).toBe('#email');
    expect(targetText(nth(steps, 0).action)).toBe('');
    expect(targetText({ type: 'click', target: { selectors: [{ kind: 'role', role: 'button', name: 'Buy' }] } })).toBe(
      'button "Buy"',
    );
  });

  it('edits the value a step has, never a secret placeholder', () => {
    const steps = sample().steps;
    expect(stepValue(nth(steps, 0).action)).toBe('http://shop.test/');
    expect(stepValue(nth(steps, 1).action)).toBeNull();
    expect(stepValue(nth(steps, 2).action)).toBe('ada@shop.test');
    expect(stepValue(nth(steps, 3).action)).toBeNull();
    expect(stepValue(nth(steps, 4).action)).toBe('Welcome');
    const edited = setStepValue(sample(), 5, 'Welcome back');
    expect(nth(edited.steps, 4).action).toEqual({
      type: 'assert',
      assertion: { kind: 'text', target: css('h1'), text: 'Welcome back' },
    });
    // A step without a value, and the secret, stay as they are.
    expect(setStepValue(sample(), 4, 'hunter2')).toEqual(sample());
    expect(setStepValue(sample(), 2, 'x')).toEqual(sample());
  });

  it('renames, deletes and moves steps without touching the original', () => {
    const original = sample();
    const renamed = renameStep(original, 1, '  Open the shop ');
    expect(nth(renamed.steps, 0).name).toBe('Open the shop');
    expect('name' in nth(renameStep(original, 2, '   ').steps, 1)).toBe(false);
    expect(deleteStep(original, 2).steps.map((step) => step.id)).toEqual([1, 3, 4, 5]);
    expect(moveStep(original, 3, -1).steps.map((step) => step.id)).toEqual([1, 3, 2, 4, 5]);
    expect(moveStep(original, 3, 1).steps.map((step) => step.id)).toEqual([1, 2, 4, 3, 5]);
    // At the ends nothing moves.
    expect(moveStep(original, 1, -1)).toEqual(original);
    expect(moveStep(original, 5, 1)).toEqual(original);
    expect(original).toEqual(sample());
  });
});
