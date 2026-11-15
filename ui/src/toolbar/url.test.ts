import { describe, expect, it } from 'vitest';
import { normalizeUrlInput } from './url';

describe('the address bar input', () => {
  it.each([
    ['http://localhost:5173/cart', 'http://localhost:5173/cart'],
    ['  https://example.com  ', 'https://example.com/'],
    ['localhost:5173', 'http://localhost:5173/'],
    ['localhost', 'http://localhost/'],
    ['127.0.0.1:8080/a?b=1', 'http://127.0.0.1:8080/a?b=1'],
    ['[::1]:3000', 'http://[::1]:3000/'],
    ['shop.localhost', 'http://shop.localhost/'],
    ['shop.test/path', 'http://shop.test/path'],
    ['web:3000', 'http://web:3000/'],
    ['example.com', 'https://example.com/'],
    ['docs.example.com/a b', 'https://docs.example.com/a%20b'],
    ['about:blank', 'about:blank'],
    ['file:///C:/work/index.html', 'file:///C:/work/index.html'],
  ])('%s opens %s', (input, url) => {
    expect(normalizeUrlInput(input)).toEqual({ ok: true, url });
  });

  it.each([
    ['', 'empty'],
    ['   ', 'empty'],
    ['javascript:alert(1)', 'unsupported'],
    ['data:text/html,hi', 'unsupported'],
    ['just words', 'invalid'],
    ['intranet', 'invalid'],
  ])('%j is refused as %s', (input, reason) => {
    expect(normalizeUrlInput(input)).toEqual({ ok: false, reason });
  });
});
