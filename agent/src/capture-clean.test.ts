// The capture preparation's page script (RB-080-12, `crates/rb-visual/src/prepare/clean.js`):
// it hides, freezes and puts the page back exactly as it was. rBrowser evaluates it in its own
// isolated world; here it runs in happy-dom's window, which has no CSS animations, so the
// animation half is covered by the e2e run (`scripts/capture-clean-e2e.mjs`).
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import cleanSource from '../../crates/rb-visual/src/prepare/clean.js?raw';

interface Answer {
  token: number;
  rules: number;
  matched: number;
  animations: number;
  videos: number;
}

interface Clean {
  prepare(options: { hide: boolean; selectors: string[]; freeze: boolean }): Promise<Answer>;
  undo(token: number): boolean;
}

function clean(): Clean {
  const scope = globalThis as unknown as { __rbCaptureClean?: Clean };
  if (!scope.__rbCaptureClean) {
    // The script is an expression statement, as `Runtime.evaluate` takes it.
    new Function(cleanSource)();
  }
  const api = scope.__rbCaptureClean;
  if (!api) throw new Error('the script defined no __rbCaptureClean');
  return api;
}

const PAGE = `
  <div id="onetrust-banner-sdk">We use cookies</div>
  <div id="own-promo">Promo</div>
  <main id="content">Content</main>
  <video id="clip"></video>
`;

let before = '';

beforeEach(() => {
  document.documentElement.setAttribute('style', 'overflow: hidden');
  document.body.innerHTML = PAGE;
  before = document.documentElement.outerHTML;
});

afterEach(() => {
  document.documentElement.removeAttribute('style');
  document.body.innerHTML = '';
});

describe('capture clean', () => {
  it('hides the selectors with one style element and lifts the scroll lock', async () => {
    const answer = await clean().prepare({
      hide: true,
      selectors: ['#onetrust-banner-sdk', '#own-promo'],
      freeze: false,
    });
    expect(answer.rules).toBe(2);
    expect(answer.matched).toBe(2);
    const styles = document.querySelectorAll(`style[data-rbrowser-capture="${answer.token}"]`);
    expect(styles).toHaveLength(1);
    const banner = document.getElementById('onetrust-banner-sdk');
    expect(banner && getComputedStyle(banner).display).toBe('none');
    const content = document.getElementById('content');
    expect(content && getComputedStyle(content).display).not.toBe('none');
    expect(document.documentElement.style.getPropertyValue('overflow-y')).toBe('visible');
    expect(clean().undo(answer.token)).toBe(true);
    expect(document.documentElement.outerHTML).toBe(before);
  });

  it('skips a selector the page cannot parse and keeps the rest', async () => {
    const answer = await clean().prepare({
      hide: true,
      selectors: ['a[', '#own-promo'],
      freeze: false,
    });
    expect(answer.matched).toBe(1);
    const promo = document.getElementById('own-promo');
    expect(promo && getComputedStyle(promo).display).toBe('none');
    clean().undo(answer.token);
    expect(document.documentElement.outerHTML).toBe(before);
  });

  it('leaves the scroll alone when no banner matched', async () => {
    const answer = await clean().prepare({ hide: true, selectors: ['#none'], freeze: false });
    expect(answer.matched).toBe(0);
    expect(document.documentElement.getAttribute('style')).toBe('overflow: hidden');
    clean().undo(answer.token);
    expect(document.documentElement.outerHTML).toBe(before);
  });

  it('pauses a playing video on its first frame and plays it again where it was', async () => {
    const video = document.getElementById('clip') as HTMLVideoElement;
    await video.play().catch(() => undefined);
    video.currentTime = 2;
    const wasPaused = video.paused;
    const answer = await clean().prepare({ hide: false, selectors: [], freeze: true });
    expect(answer.videos).toBe(1);
    expect(video.paused).toBe(true);
    expect(video.currentTime).toBe(0);
    clean().undo(answer.token);
    expect(video.currentTime).toBe(2);
    expect(video.paused).toBe(wasPaused);
    expect(document.querySelectorAll('style[data-rbrowser-capture]')).toHaveLength(0);
  });

  it('nests: a capture during the preview is undone without ending the preview', async () => {
    const preview = await clean().prepare({ hide: true, selectors: ['#own-promo'], freeze: true });
    const shot = await clean().prepare({ hide: true, selectors: ['#own-promo'], freeze: true });
    expect(shot.token).toBeGreaterThan(preview.token);
    clean().undo(shot.token);
    const promo = document.getElementById('own-promo');
    expect(promo && getComputedStyle(promo).display).toBe('none');
    clean().undo(preview.token);
    expect(document.documentElement.outerHTML).toBe(before);
  });

  it('removes a lost preparation style by its token', () => {
    const style = document.createElement('style');
    style.setAttribute('data-rbrowser-capture', '999');
    document.head.appendChild(style);
    expect(clean().undo(999)).toBe(false);
    expect(document.querySelectorAll('style[data-rbrowser-capture]')).toHaveLength(0);
  });
});
