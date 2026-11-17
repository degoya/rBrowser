import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  MAX_MISSED_ORIGINS,
  MissedOrigins,
  type MissWindow,
  elementUrl,
  isLoopbackOrigin,
  isMissedEntry,
  originOf,
  scanDocument,
  watchMisses,
} from './misses';

const PAGE = 'http://192.168.1.20:4000/shop';

describe('a missed resource', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('is named by its origin only, http or https', () => {
    expect(originOf('http://localhost:5173/storage/a.png?token=secret#x', PAGE)).toBe('http://localhost:5173');
    expect(originOf('/img/a.png', PAGE)).toBe('http://192.168.1.20:4000');
    expect(originOf('HTTPS://CDN.Example.com:443/a.css', PAGE)).toBe('https://cdn.example.com');
    expect(originOf('data:image/png;base64,AAAA', PAGE)).toBeNull();
    expect(originOf('blob:http://192.168.1.20:4000/1', PAGE)).toBeNull();
    expect(originOf('http://[', PAGE)).toBeNull();
  });

  it('knows the loopback origins a device can never reach', () => {
    expect(isLoopbackOrigin('http://localhost:5173')).toBe(true);
    expect(isLoopbackOrigin('http://127.0.0.1:8000')).toBe(true);
    expect(isLoopbackOrigin('http://[::1]:5173')).toBe(true);
    expect(isLoopbackOrigin('http://project.test')).toBe(false);
    expect(isLoopbackOrigin('http://192.168.1.10:8080')).toBe(false);
  });

  it('is the URL of the element that failed, of the loading kinds only', () => {
    const img = document.createElement('img');
    img.src = 'http://localhost:5173/a.png';
    expect(elementUrl(img)).toBe('http://localhost:5173/a.png');
    const link = document.createElement('link');
    link.href = 'http://localhost:5173/app.css';
    expect(elementUrl(link)).toBe('http://localhost:5173/app.css');
    const anchor = document.createElement('a');
    anchor.href = 'http://localhost:5173/';
    expect(elementUrl(anchor)).toBeNull();
    expect(elementUrl(window)).toBeNull();
    expect(elementUrl(null)).toBeNull();
    expect(elementUrl(document.createElement('img'))).toBeNull();
  });

  it('reports each origin once, never the page own, at most the cap', () => {
    const reported: string[] = [];
    const vias: Array<string | undefined> = [];
    const misses = new MissedOrigins('http://192.168.1.20:4000', (origin, via) => {
      reported.push(origin);
      vias.push(via);
    });
    misses.note('http://localhost:5173/a.png', PAGE, 'img');
    misses.note('http://localhost:5173/b.png', PAGE);
    misses.note('/missing.png', PAGE);
    misses.note('data:,x', PAGE);
    misses.note('http://project.test/c.png', PAGE, 'Not A Word!');
    expect(reported).toEqual(['http://localhost:5173', 'http://project.test']);
    expect(vias).toEqual(['img', undefined]);
    for (let port = 1; port <= MAX_MISSED_ORIGINS + 4; port += 1) misses.note(`http://127.0.0.1:${port}/x`, PAGE);
    expect(reported).toHaveLength(MAX_MISSED_ORIGINS);
  });

  it('counts a resource entry of a loopback origin or with an error status', () => {
    expect(isMissedEntry({ name: 'http://localhost:5173/a.png' }, PAGE)).toBe(true);
    expect(isMissedEntry({ name: 'http://localhost:5173/a.png', responseStatus: 200 }, PAGE)).toBe(true);
    expect(isMissedEntry({ name: 'http://project.test/a.png', responseStatus: 404 }, PAGE)).toBe(true);
    expect(isMissedEntry({ name: 'http://project.test/a.png', responseStatus: 200 }, PAGE)).toBe(false);
    // 0 is also what an opaque cross-origin answer reports: no evidence on its own.
    expect(isMissedEntry({ name: 'https://cdn.example.com/a.css', responseStatus: 0 }, PAGE)).toBe(false);
    expect(isMissedEntry({ name: 'https://cdn.example.com/a.css' }, PAGE)).toBe(false);
  });

  it('finds what failed before the agent ran: loopback URLs and broken images', () => {
    document.body.innerHTML =
      '<img id="broken" src="http://192.168.1.20:4000/gone.png"><img id="fine" src="http://192.168.1.20:4000/ok.png">' +
      '<audio src="http://localhost:5173/intro.mp3"></audio><a href="http://localhost:9/">a link</a>';
    const broken = document.getElementById('broken')!;
    Object.defineProperty(broken, 'complete', { value: true });
    Object.defineProperty(broken, 'naturalWidth', { value: 0 });
    const fine = document.getElementById('fine')!;
    Object.defineProperty(fine, 'complete', { value: true });
    Object.defineProperty(fine, 'naturalWidth', { value: 40 });
    expect(scanDocument(document)).toEqual([
      ['http://192.168.1.20:4000/gone.png', 'img'],
      ['http://localhost:5173/intro.mp3', 'audio'],
    ]);
  });
});

/** A window with the watch's parts, each recorded. */
function fakeWindow(): MissWindow & {
  listeners: Array<[string, (event: Event) => void, boolean]>;
  observe: (entries: Array<{ name: string; responseStatus?: number; initiatorType?: string }>) => void;
} {
  let observer: ((list: { getEntries(): unknown[] }) => void) | null = null;
  class Observer {
    constructor(callback: (list: { getEntries(): unknown[] }) => void) {
      observer = callback;
    }
    observe(): void {}
  }
  const listeners: Array<[string, (event: Event) => void, boolean]> = [];
  return {
    listeners,
    observe: (entries) => observer?.({ getEntries: () => entries }),
    document,
    addEventListener: (type: string, listener: (event: Event) => void, capture?: boolean) => {
      listeners.push([type, listener, capture === true]);
    },
    PerformanceObserver: Observer as unknown as typeof PerformanceObserver,
  };
}

describe('the watch', () => {
  it('hears element errors in the capture phase and resource entries', () => {
    const win = fakeWindow();
    const noted: Array<[string, string]> = [];
    watchMisses(win, (url, via) => noted.push([url, via]));
    const [type, onError, capture] = win.listeners.find(([kind]) => kind === 'error')!;
    expect([type, capture]).toEqual(['error', true]);
    const img = document.createElement('img');
    img.src = 'http://localhost:5173/a.png';
    onError({ target: img } as unknown as Event);
    win.observe([
      { name: 'http://127.0.0.1:9000/data.json', responseStatus: 0, initiatorType: 'fetch' },
      { name: 'http://192.168.1.20:4000/ok.css', responseStatus: 200 },
    ]);
    expect(noted).toEqual([
      ['http://localhost:5173/a.png', 'img'],
      ['http://127.0.0.1:9000/data.json', 'fetch'],
    ]);
  });

  it('notes a fetch that fails but not one aborted, and hands the page its own promise', async () => {
    const win = fakeWindow();
    const failed = Promise.reject(new TypeError('Failed to fetch'));
    const aborted = Promise.reject(Object.assign(new Error('aborted'), { name: 'AbortError' }));
    failed.catch(() => {});
    aborted.catch(() => {});
    const answers = [failed, aborted];
    win.fetch = vi.fn(() => answers.shift()!) as unknown as typeof fetch;
    const noted: Array<[string, string]> = [];
    watchMisses(win, (url, via) => noted.push([url, via]));
    const first = win.fetch!('http://localhost:8000/api/items');
    expect(first).toBe(failed);
    await expect(first).rejects.toThrow('Failed to fetch');
    await expect(win.fetch!(new URL('http://localhost:8001/x'))).rejects.toThrow('aborted');
    await Promise.resolve();
    expect(noted).toEqual([['http://localhost:8000/api/items', 'fetch']]);
  });

  it('notes an XMLHttpRequest that errs', () => {
    const win = fakeWindow();
    class FakeXhr extends EventTarget {
      opened: unknown[] = [];
      open(...args: unknown[]): void {
        this.opened = args;
      }
    }
    win.XMLHttpRequest = FakeXhr as unknown as typeof XMLHttpRequest;
    const noted: Array<[string, string]> = [];
    watchMisses(win, (url, via) => noted.push([url, via]));
    const xhr = new FakeXhr();
    xhr.open('GET', 'http://localhost:8000/api/cart');
    expect(xhr.opened).toEqual(['GET', 'http://localhost:8000/api/cart']);
    xhr.dispatchEvent(new Event('error'));
    expect(noted).toEqual([['http://localhost:8000/api/cart', 'xmlhttprequest']]);
  });
});
