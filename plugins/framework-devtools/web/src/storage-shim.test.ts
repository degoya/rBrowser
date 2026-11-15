import { afterEach, describe, expect, it } from 'vitest';
import { ensureStorage } from './storage-shim';
import vuePanel from '../../panel/dist/vue.js?raw';
import vueHtml from '../../panel/vue.html?raw';

// RB-080-66: the panels run in frames sandboxed without `allow-same-origin`, where reading
// `localStorage` throws a SecurityError; the Vue DevTools kit reads it while it loads, so the
// panel died with "Uncaught SecurityError" before it drew anything.

type Win = Window & typeof globalThis;

function sandboxed(target: Win): () => void {
  const saved = ['localStorage', 'sessionStorage'].map(
    (name) => [name, Object.getOwnPropertyDescriptor(target, name)] as const,
  );
  for (const [name] of saved) {
    Object.defineProperty(target, name, {
      configurable: true,
      get() {
        throw new DOMException(`Failed to read the '${name}' property from 'Window'`, 'SecurityError');
      },
    });
  }
  return () => {
    for (const [name, descriptor] of saved) {
      if (descriptor) Object.defineProperty(target, name, descriptor);
      else delete (target as unknown as Record<string, unknown>)[name];
    }
  };
}

let restore: () => void = () => undefined;

afterEach(() => {
  restore();
  restore = () => undefined;
});

describe('the storage of a sandboxed panel', () => {
  it('is an in-memory storage when the sandbox refuses the real one', () => {
    restore = sandboxed(window);
    ensureStorage(window);
    localStorage.setItem('a', '1');
    expect(localStorage.getItem('a')).toBe('1');
    expect(localStorage.getItem('none')).toBeNull();
    expect(localStorage.length).toBe(1);
    expect(localStorage.key(0)).toBe('a');
    localStorage.removeItem('a');
    expect(localStorage.length).toBe(0);
    sessionStorage.setItem('b', '2');
    sessionStorage.clear();
    expect(sessionStorage.getItem('b')).toBeNull();
  });

  it('leaves a readable storage as it is', () => {
    const before = window.localStorage;
    ensureStorage(window);
    expect(window.localStorage).toBe(before);
  });

  it('lets the Vue panel bundle load in a sandboxed frame', () => {
    restore = sandboxed(window);
    const page = vueHtml.replace(/<script[\s\S]*?<\/script>|<link[^>]*>/g, '');
    document.body.innerHTML = new DOMParser().parseFromString(page, 'text/html').body.innerHTML;
    expect(() => new Function(vuePanel)()).not.toThrow();
    expect(localStorage.getItem('missing')).toBeNull();
  });
});
