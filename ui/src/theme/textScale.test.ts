import { describe, expect, it } from 'vitest';
import { applyTextScale, BASE_FONT_PX, parseTextScale, remScale } from './textScale';

describe("Windows' text size (RB-090-26)", () => {
  it('takes a percentage from 100 to 225, 100 for anything else', () => {
    expect(parseTextScale(100)).toBe(100);
    expect(parseTextScale(150)).toBe(150);
    expect(parseTextScale(225)).toBe(225);
    expect(parseTextScale(99)).toBe(100);
    expect(parseTextScale(300)).toBe(225);
    expect(parseTextScale(Number.NaN)).toBe(100);
    expect(parseTextScale(undefined)).toBe(100);
  });

  it('sets the root font size, so every rem grows with it', () => {
    const root = document.createElement('html');
    applyTextScale(root, 150);
    expect(root.style.fontSize).toBe('150%');
    applyTextScale(root, 225);
    expect(root.style.fontSize).toBe('225%');
    // 100 %: the page's own size, nothing set.
    applyTextScale(root, 100);
    expect(root.style.fontSize).toBe('');
  });

  it('measures the scale of a rem on the page', () => {
    const root = document.createElement('html');
    const sized = (px: string) => ({ fontSize: px }) as CSSStyleDeclaration;
    expect(remScale(root, () => sized(`${BASE_FONT_PX}px`))).toBe(1);
    expect(remScale(root, () => sized('24px'))).toBe(1.5);
    expect(remScale(root, () => sized('36px'))).toBe(2.25);
    // Not laid out (a test without CSS) or not resolved to px: 1.
    expect(remScale(root, () => sized(''))).toBe(1);
    expect(remScale(root, () => sized('150%'))).toBe(1);
  });
});
