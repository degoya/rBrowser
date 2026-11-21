import { afterEach, describe, expect, it, vi } from 'vitest';
import { FLASH_ATTRIBUTE, FLASH_MS, flashAt } from './flash';

afterEach(() => {
  vi.useRealTimers();
  document.body.replaceChildren();
});

describe("the presenter's click on a viewer's page (RB-096-05)", () => {
  it('flashes a ring at the middle of the element and takes it away again', () => {
    vi.useFakeTimers();
    const button = document.createElement('button');
    document.body.append(button);
    button.getBoundingClientRect = () => ({ left: 10, top: 20, width: 40, height: 20 }) as DOMRect;
    expect(flashAt(button, document)).toBe(true);
    const ring = document.querySelector<HTMLElement>(`[${FLASH_ATTRIBUTE}]`);
    expect(ring).not.toBeNull();
    expect(ring?.style.position).toBe('fixed');
    expect(ring?.style.pointerEvents).toBe('none');
    expect(ring?.style.left).toBe('30px');
    expect(ring?.style.top).toBe('30px');
    expect(ring?.getAttribute('aria-hidden')).toBe('true');
    vi.advanceTimersByTime(FLASH_MS);
    expect(document.querySelector(`[${FLASH_ATTRIBUTE}]`)).toBeNull();
  });

  it('needs a page to draw on', () => {
    const element = document.createElement('div');
    const empty = { body: null, documentElement: null } as unknown as Document;
    expect(flashAt(element, empty)).toBe(false);
  });
});
