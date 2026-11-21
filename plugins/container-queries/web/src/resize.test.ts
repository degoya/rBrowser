import { beforeEach, describe, expect, it } from 'vitest';
import { ORIGINAL_ATTRIBUTE, isResized, resetAll, resetElement, resizeElement } from './resize';

let box: HTMLElement;

/** The inline declarations of `element`, in a fixed order: what a reset must bring back. */
const declarations = (element: HTMLElement): string[] =>
  Array.from(element.style, (name) => `${name}: ${element.style.getPropertyValue(name)} ${element.style.getPropertyPriority(name)}`).sort();

beforeEach(() => {
  document.body.innerHTML = '<div id="a" style="width: 50% !important; color: red"></div><div id="b"></div>';
  box = document.getElementById('a') as HTMLElement;
});

describe('a container resized by rBrowser', () => {
  it('pins its width inline and remembers the inline style it had', () => {
    resizeElement(box, 400.4);
    expect(box.style.getPropertyValue('width')).toBe('400px');
    expect(box.style.getPropertyPriority('width')).toBe('important');
    expect(box.style.getPropertyValue('max-width')).toBe('400px');
    expect(box.style.getPropertyValue('min-width')).toBe('400px');
    expect(isResized(box)).toBe(true);
    expect(box.hasAttribute(ORIGINAL_ATTRIBUTE)).toBe(true);
  });

  it('sets the content box under border-box: the padding and border come on top', () => {
    const card = document.getElementById('b') as HTMLElement;
    card.style.setProperty('box-sizing', 'border-box');
    card.style.setProperty('padding', '0 16px');
    card.style.setProperty('border', '1px solid black');
    const before = declarations(card);
    resizeElement(card, 400);
    expect(card.style.getPropertyValue('width')).toBe('434px');
    expect(card.style.getPropertyValue('min-width')).toBe('434px');
    expect(card.style.getPropertyValue('max-width')).toBe('434px');
    resetElement(card);
    expect(declarations(card)).toEqual(before);
  });

  it('keeps the first original over several drags and restores it exactly', () => {
    const before = declarations(box);
    resizeElement(box, 300);
    resizeElement(box, 700);
    expect(resetElement(box)).toBe(true);
    expect(declarations(box)).toEqual(before);
    expect(box.style.getPropertyPriority('width')).toBe('important');
    expect(isResized(box)).toBe(false);
    expect(resetElement(box)).toBe(false);
  });

  it('leaves no style attribute behind on an element that had none', () => {
    const plain = document.getElementById('b') as HTMLElement;
    resizeElement(plain, 120);
    resetElement(plain);
    expect(plain.hasAttribute('style')).toBe(false);
  });

  it('resets every resized element of the document', () => {
    resizeElement(box, 10);
    resizeElement(document.getElementById('b') as HTMLElement, 20);
    expect(resetAll(document)).toBe(2);
    expect(document.querySelectorAll(`[${ORIGINAL_ATTRIBUTE}]`)).toHaveLength(0);
  });

  it('refuses a width that is no finite number within bounds', () => {
    expect(() => resizeElement(box, Number.NaN)).toThrow();
    expect(() => resizeElement(box, -1)).toThrow();
    expect(() => resizeElement(box, 100_001)).toThrow();
    expect(isResized(box)).toBe(false);
  });

  it('ignores an original it cannot read and only drops the mark', () => {
    box.setAttribute(ORIGINAL_ATTRIBUTE, 'not json');
    expect(resetElement(box)).toBe(true);
    expect(isResized(box)).toBe(false);
  });
});
