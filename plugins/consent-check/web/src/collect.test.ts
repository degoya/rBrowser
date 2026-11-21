// The consent check's collector (src/collect.js, RB-094-05) run in a page: the configuration the
// component writes into it (data/cmp.json), the banner it finds, the storage keys (never the
// values) and the "Reject all" click of the probe session.
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterEach, describe, expect, it } from 'vitest';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const SCRIPT = readFileSync(join(ROOT, 'src', 'collect.js'), 'utf8');
const DATA = JSON.parse(readFileSync(join(ROOT, 'data', 'cmp.json'), 'utf8')) as {
  cmps: { id: string; banner: string[]; reject: string[] }[];
  rejectTexts: string[];
};

interface Result {
  url: string;
  storage: string[];
  storageCut: boolean;
  banner: { cmp: string | null; visible: boolean; rejectable: boolean };
  clicked: boolean;
}

function run(mode: 'read' | 'reject'): Result {
  const config = { mode, cmps: DATA.cmps.map(({ id, banner, reject }) => ({ id, banner, reject })), rejectTexts: DATA.rejectTexts };
  const expression = SCRIPT.replace('__CONSENT_CONFIG__', JSON.stringify(config));
  // An indirect eval: the expression as `evaluate` runs it, in the global scope.
  return (0, eval)(expression) as Result;
}

afterEach(() => {
  document.body.innerHTML = '';
  localStorage.clear();
});

describe('the collector', () => {
  it('names the storage keys, never their values', () => {
    localStorage.setItem('_track_id', 'secret-value-123');
    localStorage.setItem('theme', 'dark');
    const result = run('read');
    expect(result.storage.sort()).toEqual(['_track_id', 'theme']);
    expect(JSON.stringify(result)).not.toContain('secret-value-123');
    expect(result.storageCut).toBe(false);
  });

  it('finds a known banner and its reject button without clicking it', () => {
    let clicks = 0;
    document.body.innerHTML = '<div id="onetrust-banner-sdk"><button id="onetrust-reject-all-handler">Reject All</button></div>';
    document.getElementById('onetrust-reject-all-handler')?.addEventListener('click', () => (clicks += 1));
    const result = run('read');
    expect(result.banner).toEqual({ cmp: 'onetrust', visible: true, rejectable: true });
    expect(result.clicked).toBe(false);
    expect(clicks).toBe(0);
  });

  it('clicks "Reject all" in reject mode', () => {
    let clicks = 0;
    document.body.innerHTML = '<div id="CybotCookiebotDialog"><button id="CybotCookiebotDialogBodyButtonDecline">Deny</button></div>';
    document.getElementById('CybotCookiebotDialogBodyButtonDecline')?.addEventListener('click', () => (clicks += 1));
    const result = run('reject');
    expect(result.banner.cmp).toBe('cookiebot');
    expect(result.clicked).toBe(true);
    expect(clicks).toBe(1);
  });

  it('rejects a banner of an unknown manager by its button text, preferring "alle ablehnen" over "ablehnen"', () => {
    const clicked: string[] = [];
    document.body.innerHTML =
      '<section class="site-cookie-banner"><button id="b1">Accept all</button><button id="b2">Ablehnen</button><a id="b3" href="#">Alle ablehnen</a></section>';
    for (const id of ['b1', 'b2', 'b3']) document.getElementById(id)?.addEventListener('click', (event) => {
      event.preventDefault();
      clicked.push(id);
    });
    const result = run('reject');
    expect(result.banner.cmp).toBe('generic');
    expect(clicked).toEqual(['b3']);
  });

  it('counts a hidden banner as not shown and clicks nothing', () => {
    document.body.innerHTML = '<div id="onetrust-banner-sdk" style="display: none"><button id="onetrust-reject-all-handler">Reject All</button></div>';
    const result = run('reject');
    expect(result.banner).toEqual({ cmp: 'onetrust', visible: false, rejectable: false });
    expect(result.clicked).toBe(false);
  });

  it('reports no banner on a page without one', () => {
    const result = run('read');
    expect(result.banner).toEqual({ cmp: null, visible: false, rejectable: false });
    expect(result.url).toBe(location.href);
  });
});
