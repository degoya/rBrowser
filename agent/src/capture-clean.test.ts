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
  prepare(options: {
    hide: boolean;
    selectors: string[];
    freeze: boolean;
    mask?: boolean;
  }): Promise<Answer>;
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

  // RB-093-04: a bug report's screenshots black out the password fields, then put the page back.
  it('masks the password fields with one rule and takes it away again', async () => {
    document.body.insertAdjacentHTML(
      'beforeend',
      '<input id="pw" type="password" value="hunter22"><input id="shown" autocomplete="current-password">',
    );
    const page = document.documentElement.outerHTML;
    const answer = await clean().prepare({ hide: false, selectors: [], freeze: false, mask: true });
    const style = document.querySelector(`style[data-rbrowser-capture="${answer.token}"]`) as HTMLStyleElement;
    const rules = Array.from(style.sheet?.cssRules ?? [], (rule) => rule.cssText);
    expect(rules).toHaveLength(1);
    expect(rules[0]).toContain('password');
    expect(rules[0]).toContain('transparent');
    clean().undo(answer.token);
    // The page comes back as it was; the memory of a password field stays (security review w27).
    expect(document.documentElement.outerHTML).toBe(
      page.replace('value="hunter22"', 'value="hunter22" data-rbrowser-was-password=""'),
    );
  });

  // Security review w27: a show-password toggle turns the field into text without a hint - a
  // field once seen as a password stays masked for the rest of the document.
  it('masks a password field the page turned into text after it was seen', async () => {
    document.body.insertAdjacentHTML('beforeend', '<input id="pw" type="password" value="hunter22">');
    const first = await clean().prepare({ hide: false, selectors: [], freeze: false, mask: true });
    clean().undo(first.token);
    const field = document.getElementById('pw') as HTMLInputElement;
    field.removeAttribute('data-rbrowser-was-password');
    field.type = 'text';
    await new Promise((done) => setTimeout(done, 0));
    const shot = await clean().prepare({ hide: false, selectors: [], freeze: false, mask: true });
    expect(field.matches('input[data-rbrowser-was-password]')).toBe(true);
    const style = document.querySelector(`style[data-rbrowser-capture="${shot.token}"]`) as HTMLStyleElement;
    expect(style.sheet?.cssRules[0]?.cssText).toContain('data-rbrowser-was-password');
    clean().undo(shot.token);
  });

  it('masks the fields that name a secret for the capture alone', async () => {
    document.body.insertAdjacentHTML(
      'beforeend',
      '<input id="a" name="userPassword"><input id="b" aria-label="Your PIN">' +
        '<input id="c" placeholder="API token"><input id="d" autocomplete="one-time-code">' +
        '<input id="e" name="client_secret"><input id="f" name="shipping"><input id="g" name="passport">',
    );
    const page = document.documentElement.outerHTML;
    const answer = await clean().prepare({ hide: false, selectors: [], freeze: false, mask: true });
    const masked = Array.from(document.querySelectorAll('input[data-rbrowser-mask]'), (input) => input.id);
    expect(masked).toEqual(['a', 'b', 'c', 'd', 'e']);
    const style = document.querySelector(`style[data-rbrowser-capture="${answer.token}"]`) as HTMLStyleElement;
    expect(style.sheet?.cssRules[0]?.cssText).toContain('data-rbrowser-mask');
    clean().undo(answer.token);
    expect(document.documentElement.outerHTML).toBe(page);
  });

  it('adds no mask rule unless asked', async () => {
    const answer = await clean().prepare({ hide: false, selectors: [], freeze: false });
    const style = document.querySelector(`style[data-rbrowser-capture="${answer.token}"]`) as HTMLStyleElement;
    expect(style.sheet?.cssRules).toHaveLength(0);
    clean().undo(answer.token);
  });

  it('removes a lost preparation style by its token', () => {
    const style = document.createElement('style');
    style.setAttribute('data-rbrowser-capture', '999');
    document.head.appendChild(style);
    expect(clean().undo(999)).toBe(false);
    expect(document.querySelectorAll('style[data-rbrowser-capture]')).toHaveLength(0);
  });
});
