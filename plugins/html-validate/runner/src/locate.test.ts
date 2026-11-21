// The element of a line (RB-094-06): a message that names a line and column but no selector -
// every Nu message, html-validate's stray end tags - gets the selector of the element there, so
// the findings panel can highlight it in the view.
import { describe, expect, it } from 'vitest';
import invalid from '../../tests/fixtures/invalid.html?raw';
import { offsetOf, selectorAt, withSelectors } from './locate';
import type { AnswerMessage } from './shape';

function message(line: number, column: number, selector: string | null = null): AnswerMessage {
  return { rule: 'nu-error', severity: 'error', message: 'm', line, column, selector, extract: '' };
}

describe('offsetOf', () => {
  it('turns a 1-based line and column into an offset', () => {
    expect(offsetOf('ab\ncd\nef', 1, 1)).toBe(0);
    expect(offsetOf('ab\ncd\nef', 2, 2)).toBe(4);
    expect(offsetOf('ab\r\ncd', 2, 1)).toBe(4);
  });

  it('has no offset for a place outside the source', () => {
    expect(offsetOf('ab', 0, 1)).toBeNull();
    expect(offsetOf('ab', 3, 1)).toBeNull();
  });
});

describe('selectorAt', () => {
  it('names the element whose start tag holds the place', () => {
    // Line 10: `<p id="intro">Second</p>`, the second paragraph of main.
    expect(selectorAt(invalid, 10, 8)).toBe('html > body > main > p:nth-child(2)');
    // Line 12: `<ul><p>...`: column 6 is in the `<p>`.
    expect(selectorAt(invalid, 12, 6)).toBe('html > body > main > ul > p');
  });

  it('names the element before a place in text or an end tag', () => {
    // Line 11: `<p><div>Block in a paragraph</div></p>`, column 30 is in the div's text.
    expect(selectorAt(invalid, 11, 30)).toBe('html > body > main > div');
  });

  it('has none before the first element or outside the source', () => {
    expect(selectorAt(invalid, 1, 1)).toBeNull();
    expect(selectorAt(invalid, 99, 1)).toBeNull();
    expect(selectorAt('', 1, 1)).toBeNull();
  });
});

describe('withSelectors', () => {
  it('fills in the selector of a message that names a line only', () => {
    const [placed, kept, document] = withSelectors(invalid, [
      message(10, 8),
      message(12, 6, '#given'),
      message(0, 0),
    ]);
    expect(placed?.selector).toBe('html > body > main > p:nth-child(2)');
    expect(kept?.selector).toBe('#given');
    expect(document?.selector).toBeNull();
  });

  it('leaves the messages alone when every one has a selector or no line', () => {
    const messages = [message(10, 8, '#a'), message(0, 0)];
    expect(withSelectors(invalid, messages)).toEqual(messages);
  });

  it('takes the place to look at from the lookup when one is given', () => {
    const [placed] = withSelectors(invalid, [message(10, 1)], [{ line: 10, column: 15 }]);
    expect(placed?.selector).toBe('html > body > main > p:nth-child(2)');
  });
});
