// The SEO panel in its frame (RB-080-85): mounted as the panel frame mounts it, answered with the
// snapshot of the fixture `social-ok` (tests/groups.rs writes it) and the plugin's catalogue as
// the panels window sends them. The panels window is played by spying on postMessage and
// dispatching `message` events from window.parent (the test window itself).
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { mountPanel } from '../../../../web-common/panel-ui/src/mount';
import App from './App.vue';
import type { Card, Preview } from './model';
import type { Serp } from './serp';

type Posted = { type?: string; id?: number; tool?: string; args?: Record<string, unknown>; action?: string; url?: string };

const here = dirname(fileURLToPath(import.meta.url));
const plugin = join(here, '..', '..', '..');

function flatten(node: unknown, prefix = ''): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [key, value] of Object.entries(node as Record<string, unknown>)) {
    const path = prefix ? `${prefix}.${key}` : key;
    if (typeof value === 'string') out[path] = value;
    else Object.assign(out, flatten(value, path));
  }
  return out;
}

const STRINGS = { ...flatten(JSON.parse(readFileSync(join(plugin, 'locales', 'en.json'), 'utf8'))), 'plugins.panel.call_failed': 'The plugin could not do that.' };
const ANSWER = JSON.parse(readFileSync(join(plugin, 'tests', 'snapshots', 'social-ok.preview.json'), 'utf8')) as Preview;

let posted: Posted[] = [];
let unmount: () => void = () => undefined;

const $ = (selector: string): HTMLElement | null => document.querySelector<HTMLElement>(selector);
const card = (id: string): HTMLElement => $(`#cards [data-platform="${id}"]`) as HTMLElement;
const settle = async (): Promise<void> => {
  await new Promise((resolve) => setTimeout(resolve, 0));
  await nextTick();
};

async function fromHost(data: unknown): Promise<void> {
  window.dispatchEvent(new MessageEvent('message', { data, source: window }));
  await settle();
}

async function answer(value: unknown, ok = true): Promise<void> {
  for (const message of posted.filter((m) => m.type === 'call' && m.tool === 'social_preview')) {
    await fromHost(ok ? { type: 'result', id: message.id, ok: true, value } : { type: 'result', id: message.id, ok: false, error: value });
  }
}

/** The fixture's answer with the cards changed. */
function withCards(change: (cards: Card[]) => Card[]): Preview {
  return { ...ANSWER, cards: change(structuredClone(ANSWER.cards)) };
}

/** The fixture's answer with its search result changed. */
function withSerp(change: Partial<Serp>): Preview {
  return { ...ANSWER, serp: { ...(ANSWER.serp as Serp), ...change } };
}

const LONG = 'A title far longer than any search result line can hold, so the search has to cut it somewhere near the middle of the text';

beforeEach(async () => {
  posted = [];
  document.body.innerHTML = '<div id="app"></div>';
  vi.spyOn(window, 'postMessage').mockImplementation((message: unknown) => {
    posted.push(message as Posted);
  });
  unmount = mountPanel(App, { prefix: 'plugin.seo.', name: 'seo panel' }).unmount;
  await fromHost({ type: 'settings', strings: STRINGS, settings: {} });
});

afterEach(() => {
  unmount();
  vi.restoreAllMocks();
});

describe('the SEO panel', () => {
  it('has its title with an icon and Refresh as an icon button with its name', () => {
    expect($('#title')?.textContent).toBe('Social previews');
    const refresh = $('#refresh') as HTMLButtonElement;
    expect(refresh.getAttribute('aria-label')).toBe('Refresh');
    expect(refresh.textContent).toBe('');
  });

  it('says it is loading, then shows the page and a card per platform in the table order', async () => {
    expect($('#status')?.textContent).toBe('Loading the previews...');
    expect($('#status')?.dataset.tone).toBe('busy');
    await answer(ANSWER);
    expect($('#status')?.textContent).toBe('');
    expect($('#page')?.textContent).toContain('Page: ');
    expect($('#page [data-count]')?.textContent).toBe('7 platforms');
    const ids = [...document.querySelectorAll<HTMLElement>('#cards [data-platform]')].map((one) => one.dataset.platform);
    expect(ids).toEqual(['facebook', 'x', 'linkedin', 'slack', 'whatsapp', 'discord', 'imessage']);
    const facebook = card('facebook');
    expect(facebook.querySelector('h2')?.textContent).toBe('Facebook');
    expect(facebook.getAttribute('aria-labelledby')).toBe(facebook.querySelector('h2')?.id);
  });

  it('shows the texts as the component cut them, the image from its data: URI and its size', async () => {
    await answer(ANSWER);
    const whatsapp = card('whatsapp');
    const title = whatsapp.querySelector<HTMLElement>('[data-title]');
    expect(title?.textContent?.endsWith('…')).toBe(true);
    expect(title?.title).toContain('(from og:title)');
    expect(whatsapp.querySelector('[data-cut]')?.textContent).toBe('Cut: the full text has 79 characters.');
    expect(card('linkedin').querySelector('[data-description]')).toBeNull();
    expect(card('facebook').querySelector('img')?.getAttribute('src')).toMatch(/^data:image\/jpeg;base64,/);
    expect(card('facebook').querySelector('[data-size]')?.textContent).toBe('1200 x 630 image/jpeg');
    expect(card('facebook').querySelector('[data-domain]')?.textContent).toBe('fixture.test');
  });

  it('names the variant its rule picks and why, with how sure the rule is', async () => {
    await answer(ANSWER);
    const facebook = card('facebook');
    expect(facebook.querySelector('[data-layout]')?.textContent).toBe('Large image card');
    expect(facebook.querySelector('[data-preview]')?.getAttribute('data-layout')).toBe('large');
    expect(facebook.querySelector('[data-reason]')?.textContent).toBe("The share image (1200 x 630) meets the platform's rule for a large card.");
    expect(facebook.querySelector('[data-confidence]')?.textContent).toBe('Documented rule');
    expect(card('x').querySelector('[data-reason]')?.textContent).toBe('twitter:card is summary_large_image.');
  });

  it('shows the other variant below on the toggle, the chosen one stays', async () => {
    await answer(
      withCards((cards) =>
        cards.map((one) =>
          one.id === 'facebook'
            ? { ...one, layout: 'small', alternate: 'large', reason: { code: 'below', params: { width: '400', height: '400', min_width: '600', min_height: '315' } } }
            : one,
        ),
      ),
    );
    const facebook = card('facebook');
    expect(facebook.querySelector('[data-reason]')?.textContent).toBe('The share image (400 x 400) is below 600 x 315.');
    const toggle = facebook.querySelector('[data-toggle]') as HTMLButtonElement;
    expect(toggle.textContent).toBe('How it would look large');
    expect(toggle.getAttribute('aria-expanded')).toBe('false');
    expect(facebook.querySelector('[data-other]')).toBeNull();
    toggle.click();
    await settle();
    expect(toggle.getAttribute('aria-expanded')).toBe('true');
    expect(toggle.textContent).toBe('Hide the other variant');
    const previews = [...facebook.querySelectorAll<HTMLElement>('[data-preview]')].map((one) => one.dataset.layout);
    expect(previews).toEqual(['small', 'large']);
    expect(facebook.querySelector('[data-other]')?.textContent).toContain('Not shown: Large image card');
  });

  it('shows both variants of an unsure rule, marked unverified, without a toggle', async () => {
    await answer(
      withCards((cards) => [
        ...cards,
        { ...(cards[0] as Card), id: 'telegram', name: 'Telegram', layout: 'large', alternate: 'small', unsure: true, confidence: 'unverified', reason: { code: 'unverified', params: {} } },
      ]),
    );
    const telegram = card('telegram');
    expect(telegram.dataset.unsure).toBe('');
    expect([...telegram.querySelectorAll<HTMLElement>('[data-preview]')].map((one) => one.dataset.layout)).toEqual(['large', 'small']);
    expect([...telegram.querySelectorAll('[data-layout]')].map((one) => one.textContent).filter((text) => text !== '')).toContain('Small image card');
    expect(telegram.querySelector('[data-confidence]')?.textContent).toBe('Unverified');
    expect(telegram.querySelector('[data-toggle]')).toBeNull();
    expect(telegram.querySelector('[data-reason]')?.textContent).toBe('No published rule: both variants are shown.');
  });

  it('shows a card without an image as text alone', async () => {
    await answer(withCards((cards) => cards.map((one) => ({ ...one, image: null, layout: 'text', alternate: null, reason: { code: 'no_image', params: {} } }))));
    const x = card('x');
    expect(x.querySelector('[data-preview]')?.getAttribute('data-layout')).toBe('text');
    expect(x.querySelector('[data-image]')).toBeNull();
    expect(x.querySelector('[data-toggle]')).toBeNull();
  });

  it("opens a rule's source through the host, only on a click", async () => {
    await answer(ANSWER);
    expect(posted.some((m) => m.action === 'open_link')).toBe(false);
    (card('discord').querySelector('[data-source]') as HTMLButtonElement).click();
    expect(posted.filter((m) => m.action === 'open_link')).toEqual([{ target: 'host', action: 'open_link', url: 'https://docs.discord.com/developers/link-previews/overview' }]);
  });

  it('asks again on Refresh and for the page the command posted', async () => {
    await answer(ANSWER);
    const before = posted.filter((m) => m.tool === 'social_preview').length;
    ($('#refresh') as HTMLButtonElement).click();
    await fromHost({ type: 'event', message: { type: 'preview', url: 'https://other.test/' } });
    const calls = posted.filter((m) => m.tool === 'social_preview');
    expect(calls.length).toBe(before + 2);
    expect(calls.at(-1)?.args).toEqual({ url: 'https://other.test/', budget: 60_000 });
  });

  // Review of RB-080-85 (1): the latest request wins; an older answer neither shows nor ends the loading.
  it('shows only the answer of the latest request and keeps loading until it comes', async () => {
    await answer(ANSWER);
    ($('#refresh') as HTMLButtonElement).click();
    await fromHost({ type: 'event', message: { type: 'preview', url: 'https://b.test/' } });
    const [older, latest] = posted.filter((m) => m.tool === 'social_preview').slice(-2);
    await fromHost({ type: 'result', id: older?.id, ok: true, value: { ...ANSWER, finalUrl: 'https://old.test/' } });
    expect($('#page')?.textContent).not.toContain('https://old.test/');
    expect($('#status')?.dataset.tone).toBe('busy');
    expect(($('#refresh') as HTMLButtonElement).disabled).toBe(true);
    await fromHost({ type: 'result', id: latest?.id, ok: true, value: { ...ANSWER, finalUrl: 'https://b.test/' } });
    expect($('#page')?.textContent).toContain('https://b.test/');
    expect($('#status')?.textContent).toBe('');
    expect(($('#refresh') as HTMLButtonElement).disabled).toBe(false);
  });

  // RB-090-44: the owner's "The share image is too large to show here (169951 bytes)" on every
  // card. The panel asks for an answer as large as the panels window's message limit (the
  // setting the host sends; 64 KiB without it), so a share image of 170 KB shows.
  it('asks for answers as large as the message limit and shows a share image of 170 KB', async () => {
    await answer(ANSWER);
    expect(posted.find((m) => m.tool === 'social_preview')?.args).toEqual({ budget: 60_000 });
    await fromHost({ type: 'settings', strings: STRINGS, settings: { 'plugins.panelMessageMaxBytes': 4 * 1024 * 1024 } });
    ($('#refresh') as HTMLButtonElement).click();
    await settle();
    const latest = posted.filter((m) => m.tool === 'social_preview').at(-1);
    expect(latest?.args).toEqual({ budget: 4 * 1024 * 1024 - 5536 });
    const big = `data:image/jpeg;base64,${'A'.repeat(Math.ceil(169_951 / 3) * 4)}`;
    const images = ANSWER.images.map((image) => ({ ...image, bytes: 169_951, dataUri: big, tooLarge: false }));
    await fromHost({ type: 'result', id: latest?.id, ok: true, value: { ...ANSWER, images } });
    expect(card('facebook').querySelector('img')?.getAttribute('src')).toBe(big);
    expect(document.body.textContent).not.toContain('too large to show');
  });

  it('says a failure in one sentence in the status line, never the code', async () => {
    await answer('plugins.error.busy', false);
    expect($('#status')?.textContent).toBe('The previews could not be built: The plugin could not do that.');
    expect($('#status')?.dataset.tone).toBe('error');
  });

  // RB-090-95: the page's search result above the cards, per device, cut by pixel width.
  it('shows the search result for desktop and mobile above the cards', async () => {
    await answer(ANSWER);
    const serp = $('#serp') as HTMLElement;
    expect(serp.querySelector('h2')?.textContent).toBe('Search result');
    expect(serp.compareDocumentPosition($('#cards') as HTMLElement) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect([...serp.querySelectorAll<HTMLElement>('[data-device]')].map((one) => one.dataset.device)).toEqual(['desktop', 'mobile']);
    const desktop = serp.querySelector('[data-device="desktop"]') as HTMLElement;
    expect(desktop.querySelector('h3')?.textContent).toBe('Desktop (600 px)');
    expect(desktop.querySelector('[data-site]')?.textContent).toBe('fixture.test');
    expect(desktop.querySelector('[data-breadcrumb]')?.textContent).toBe('fixture.test › social-ok');
    expect(desktop.querySelector('[data-title]')?.textContent).toBe('Social fixture: six platforms preview this page');
    expect(desktop.querySelector<HTMLElement>('[data-title]')?.title).toContain('(from title)');
    expect(desktop.querySelector('[data-measure="title"]')?.textContent).toMatch(/^Title: 47 characters, \d+ px of 600 px\.$/);
    expect(desktop.querySelector('[data-cut]')).toBeNull();
    expect(serp.querySelector('[data-device="mobile"] h3')?.textContent).toBe('Mobile (328 px on a 360 px phone)');
    expect(serp.querySelector('[data-confidence]')?.textContent).toBe('Unverified');
    expect(serp.querySelector('[data-noindex]')).toBeNull();
    // The fixture site has no favicon: a globe, and a note why.
    const placeholder = desktop.querySelector('[data-favicon]');
    expect(placeholder?.querySelector('img')).toBeNull();
    expect(placeholder?.querySelector('[title]')?.getAttribute('title')).toBe('The favicon could not be loaded (404); the search shows a neutral globe.');
    expect(serp.querySelector('[data-favicon-note]')?.textContent).toBe('The favicon could not be loaded (404); the search shows a neutral globe.');
  });

  it('cuts a long title at a word end with an ellipsis, notes it and keeps the whole text as the tooltip', async () => {
    await answer(withSerp({ title: { text: LONG, source: 'title' } }));
    const desktop = $('#serp [data-device="desktop"]') as HTMLElement;
    const title = desktop.querySelector<HTMLElement>('[data-title]');
    expect(title?.textContent?.endsWith('…')).toBe(true);
    expect(LONG.startsWith(title?.textContent?.slice(0, -1) ?? 'x')).toBe(true);
    expect(title?.title).toContain(LONG);
    expect(desktop.querySelector('[data-cut="title"]')?.textContent).toMatch(/^Cut: \d+ of 122 characters show; the whole text is in the tooltip\.$/);
    const mobile = $('#serp [data-device="mobile"]') as HTMLElement;
    expect(mobile.querySelectorAll('[data-title] span').length).toBe(2);
  });

  it('says what the search does without a title or a description, and when the page says noindex', async () => {
    await answer(withSerp({ title: null, description: null, noindex: true }));
    const desktop = $('#serp [data-device="desktop"]') as HTMLElement;
    expect(desktop.querySelector('[data-title][data-missing]')?.textContent).toBe('No title: the search makes one from the page.');
    expect(desktop.querySelector('[data-description][data-missing]')?.textContent).toBe('No meta description: the search shows a passage of the page instead.');
    expect($('#serp [data-noindex]')?.textContent).toContain('it does not show in the search like this');
  });

  it('shows the favicon from its data: URI', async () => {
    const favicon = { ...(ANSWER.serp as Serp).favicon, status: 200, format: 'image/x-icon', bytes: 4, dataUri: 'data:image/x-icon;base64,AAABAA==' } as Serp['favicon'];
    await answer(withSerp({ favicon }));
    expect($('#serp [data-favicon] img')?.getAttribute('src')).toBe('data:image/x-icon;base64,AAABAA==');
    expect($('#serp [data-favicon-note]')).toBeNull();
  });

  it('shows the devices the setting serpPreview names, none when off', async () => {
    await answer(ANSWER);
    await fromHost({ type: 'settings', strings: STRINGS, settings: { 'seo.serpPreview': 'mobile' } });
    expect([...document.querySelectorAll<HTMLElement>('#serp [data-device]')].map((one) => one.dataset.device)).toEqual(['mobile']);
    await fromHost({ type: 'settings', strings: STRINGS, settings: { 'seo.serpPreview': 'off' } });
    expect($('#serp')).toBeNull();
    expect(document.querySelectorAll('#cards [data-platform]').length).toBe(7);
  });

  it('leaves the search result out of an answer whose serp is broken, the cards stay', async () => {
    await answer({ ...ANSWER, serp: { title: 3 } });
    expect($('#serp')).toBeNull();
    expect(document.querySelectorAll('#cards [data-platform]').length).toBe(7);
  });

  it("takes the app's theme from its settings", async () => {
    await fromHost({ type: 'settings', strings: STRINGS, settings: { 'ui.theme': 'dark' } });
    expect(document.documentElement.classList.contains('dark')).toBe(true);
    await fromHost({ type: 'settings', strings: STRINGS, settings: {} });
    expect(document.documentElement.dataset.theme).toBeUndefined();
  });
});
