import { afterEach, describe, expect, it } from 'vitest';
import { returnFocus } from './returnFocus';

afterEach(() => {
  document.body.replaceChildren();
});

function button(text: string): HTMLButtonElement {
  const element = document.createElement('button');
  element.textContent = text;
  document.body.append(element);
  return element;
}

describe('returnFocus', () => {
  it('puts the focus back where it was when the layer opened', () => {
    const opener = button('opener');
    const inside = button('inside');
    opener.focus();
    const focus = returnFocus();
    focus.remember();
    inside.focus();
    focus.restore();
    expect(document.activeElement).toBe(opener);
  });

  it('keeps the first opener when the layer is asked to open again while open', () => {
    const opener = button('opener');
    const inside = button('inside');
    opener.focus();
    const focus = returnFocus();
    focus.remember();
    inside.focus();
    focus.remember();
    focus.restore();
    expect(document.activeElement).toBe(opener);
  });

  it('leaves the focus alone when the opener is gone, disabled or was the body', () => {
    const focus = returnFocus();
    const other = button('other');
    // The body: the command came from a page or a shortcut, nothing in the shell had the focus.
    (document.activeElement as HTMLElement | null)?.blur();
    focus.remember();
    other.focus();
    focus.restore();
    expect(document.activeElement).toBe(other);

    const gone = button('gone');
    gone.focus();
    focus.remember();
    other.focus();
    gone.remove();
    focus.restore();
    expect(document.activeElement).toBe(other);

    const disabled = button('disabled');
    disabled.focus();
    focus.remember();
    other.focus();
    disabled.disabled = true;
    focus.restore();
    expect(document.activeElement).toBe(other);
  });

  it('forgets the opener when the layer closes because the focus left it', () => {
    const opener = button('opener');
    const elsewhere = button('elsewhere');
    opener.focus();
    const focus = returnFocus();
    focus.remember();
    elsewhere.focus();
    focus.forget();
    focus.restore();
    expect(document.activeElement).toBe(elsewhere);
  });
});
