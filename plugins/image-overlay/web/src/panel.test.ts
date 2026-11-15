// The image-overlay panel in a window of its own: the bridge messages it sends and how it renders
// the plugin's answers. The panels window is played by spying on postMessage and dispatching
// `message` events from window.parent (the test window itself). On Nuxt UI since RB-080-86.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { mountPanel } from '../../../web-common/panel-ui/src/mount';
import App from './panel/App.vue';
import en from '../../locales/en.json';
import { state } from './support';

type Sent = { type: string; id?: number; tool?: string; command?: string; args?: Record<string, unknown> };

const IMAGE = { path: 'designs/mobile.png', mime: 'image/png', bytes: 10, hash: 'h', width: 393, height: 852, scale: 1, chunks: 1 };
const ANSWER = {
  state: state({ enabled: false, opacity: 50, blend: 'normal', designs: { mobile: 'designs/mobile.png' } }),
  views: [
    { view: 1, label: 'Mobile', breakpoint: 'mobile', image: IMAGE, problem: null },
    { view: 2, label: 'Tablet', breakpoint: 'tablet', image: IMAGE, problem: null },
    {
      view: 3,
      label: 'Desktop',
      breakpoint: 'desktop',
      image: null,
      problem: { code: 'plugin.image-overlay.errors.path_outside', params: { path: '../x.png' } },
    },
  ],
  designs: ['desktop.png', 'mobile.png'],
};
const DESKTOP = { name: 'desktop.png', path: 'designs/desktop.png', mime: 'image/png', bytes: 4, hash: 'aaaaaaaaaaaaaaaa', width: 1440, height: 900, scale: 1, chunks: 1 };
const MOBILE = { name: 'mobile.png', path: 'designs/mobile.png', mime: 'image/png', bytes: 4, hash: 'bbbbbbbbbbbbbbbb', width: 393, height: 852, scale: 1, chunks: 1 };
const LIBRARY = { designs: [DESKTOP, MOBILE], more: false, trusted: true, chunkBytes: 48128 };

let sent: Sent[] = [];
/** The plugin's texts and a host text, as the panels window sends them. */
const STRINGS = (): Record<string, string> => ({ ...flatten(en, '', {}), 'plugins.error.busy': 'The plugin is busy.' });
let unmount: () => void = () => undefined;

function flatten(value: unknown, prefix: string, into: Record<string, string>): Record<string, string> {
  if (typeof value === 'string') into[prefix] = value;
  else if (value && typeof value === 'object') {
    for (const [key, child] of Object.entries(value)) flatten(child, prefix ? `${prefix}.${key}` : key, into);
  }
  return into;
}

const settle = async (): Promise<void> => {
  for (let round = 0; round < 3; round += 1) {
    await new Promise((resolve) => setTimeout(resolve, 0));
    await nextTick();
  }
};

async function fromHost(data: unknown): Promise<void> {
  window.dispatchEvent(new MessageEvent('message', { data, source: window }));
  await settle();
}

const calls = (): Sent[] => sent.filter((message) => message.type === 'call');

/** Answers the last call with `value`, or fails it with the code `value` and `params`. */
async function answer(value: unknown, ok = true, params?: Record<string, unknown>): Promise<void> {
  const call = calls().at(-1);
  await fromHost(ok ? { type: 'result', id: call?.id, ok, value } : { type: 'result', id: call?.id, ok, error: value, params });
}

/** Answers the last call that `match` picks with `value`. */
async function reply(match: (call: Sent) => boolean, value: unknown): Promise<void> {
  const call = calls().filter(match).at(-1);
  await fromHost({ type: 'result', id: call?.id, ok: true, value });
}

const isSet = (call: Sent): boolean => call.tool === 'set';
const isList = (call: Sent): boolean => call.tool === 'designs' && Object.keys(call.args ?? {}).length === 0;

/** The component's post after a change elsewhere: the panel asks for the state and the designs. */
const reloaded = (): Promise<void> => fromHost({ type: 'event', message: { state: ANSWER.state, reload: true } });

function el<T extends HTMLElement>(selector: string): T {
  const found = document.querySelector(selector);
  if (!found) throw new Error(`no ${selector}`);
  return found as T;
}

async function key(target: HTMLElement, name: string, shiftKey = false): Promise<KeyboardEvent> {
  const event = new KeyboardEvent('keydown', { key: name, shiftKey, bubbles: true, cancelable: true });
  target.dispatchEvent(event);
  await settle();
  return event;
}

const status = (): HTMLElement => el('#status');
const select = (breakpoint: string): HTMLElement => el(`button[data-breakpoint="${breakpoint}"]`);

/** The entries a select offers, opened by keyboard. */
async function offered(trigger: HTMLElement): Promise<HTMLElement[]> {
  await key(trigger, 'Enter');
  const options = [...document.querySelectorAll<HTMLElement>('[role="option"]')];
  return options;
}

const texts = (nodes: HTMLElement[]): string[] => nodes.map((node) => (node.textContent ?? '').trim());

/** Opens the select and picks the entry that reads `label` - by keyboard, Enter on the entry. */
async function choose(trigger: HTMLElement, label: string): Promise<void> {
  const option = (await offered(trigger)).find((one) => (one.textContent ?? '').trim() === label);
  if (!option) throw new Error(`no entry ${label}`);
  option.focus();
  await key(option, 'Enter');
}

beforeEach(async () => {
  sent = [];
  document.body.innerHTML = '<div id="app"></div>';
  vi.spyOn(window, 'postMessage').mockImplementation((message: unknown) => {
    sent.push(message as Sent);
  });
  unmount = mountPanel(App, { prefix: 'plugin.image-overlay.', name: 'image-overlay panel' }).unmount;
  await fromHost({ type: 'settings', strings: STRINGS(), settings: {} });
  await answer(ANSWER);
  // The names changed: the panel asks for the designs with their sizes.
  expect(calls().at(-1)).toMatchObject({ tool: 'designs', args: {} });
  await answer(LIBRARY);
});

afterEach(() => {
  unmount();
  vi.restoreAllMocks();
});

describe('the panel', () => {
  it('asks for its texts and the state, and shows them with labels', () => {
    expect(sent[0]).toEqual({ type: 'settings' });
    expect(calls()[0]).toMatchObject({ tool: 'set', args: {} });
    expect(el('#title').textContent).toBe('Image overlay');
    expect(el('#enabled').getAttribute('aria-checked')).toBe('false');
    expect(el('#opacity [role="slider"]').getAttribute('aria-valuenow')).toBe('50');
    expect(el('#opacity-value').textContent).toBe('50 %');
    expect(el('#blend').textContent?.trim()).toBe('Normal');
    expect(el('label[for="blend"]').textContent?.trim()).toBe('Blend mode');
    const pinned = el('#anchor [role="radio"][aria-checked="true"]');
    expect(el(`label[for="${pinned.id}"]`).textContent?.trim()).toBe('Pinned to the page (scrolls with it)');
    expect(el('#anchor').textContent).toContain('Position');
    // Every control has its name.
    for (const control of document.querySelectorAll<HTMLElement>('input, button, [role="slider"], [role="switch"], [role="radio"], [role="combobox"]')) {
      const labelled = control.id ? document.querySelector(`label[for="${control.id}"]`) : null;
      const name = labelled?.textContent || control.closest('label')?.textContent || control.getAttribute('aria-label') || control.textContent?.trim();
      expect(name, control.outerHTML).toBeTruthy();
    }
  });

  it('switches the overlay and sets opacity and blend through the tool', async () => {
    el('#enabled').click();
    await settle();
    expect(calls().at(-1)).toMatchObject({ tool: 'set', args: { enabled: true } });
    await answer({ ...ANSWER, state: { ...ANSWER.state, enabled: true }, saved: true });
    expect(status().textContent).toBe('Saved.');
    expect(el('#enabled').getAttribute('aria-checked')).toBe('true');
    const opacity = el('#opacity [role="slider"]');
    await key(opacity, 'PageDown');
    expect(el('#opacity-value').textContent).toBe('40 %');
    expect(calls().at(-1)).toMatchObject({ args: { opacity: 40 } });
    await choose(el('#blend'), 'Difference');
    expect(calls().at(-1)).toMatchObject({ args: { blend: 'difference' } });
  });

  it('inverts and pins the design through the tool', async () => {
    el('#invert').click();
    await settle();
    expect(calls().at(-1)).toMatchObject({ args: { invert: true } });
    const fixed = [...document.querySelectorAll<HTMLElement>('#anchor [role="radio"]')][1] as HTMLElement;
    fixed.click();
    await settle();
    expect(calls().at(-1)).toMatchObject({ args: { anchor: 'viewport' } });
  });

  it('moves the design with the arrow keys, Shift for 10 px', async () => {
    const nudge = el('#nudge');
    const event = await key(nudge, 'ArrowRight');
    expect(event.defaultPrevented).toBe(true);
    expect(calls().at(-1)).toMatchObject({ args: { offset: { x: 1, y: 0 } } });
    await answer({ ...ANSWER, state: { ...ANSWER.state, offset: { x: 1, y: 0 } } });
    await key(el('#nudge'), 'ArrowDown', true);
    expect(calls().at(-1)).toMatchObject({ args: { offset: { x: 1, y: 10 } } });
    await answer({ ...ANSWER, state: { ...ANSWER.state, offset: { x: 1, y: 10 } } });
    expect(el<HTMLInputElement>('#offset-y').value).toBe('10');
    el('#offset-reset').click();
    expect(calls().at(-1)).toMatchObject({ args: { offset: { x: 0, y: 0 } } });
  });

  it('takes a typed offset', async () => {
    const x = el<HTMLInputElement>('#offset-x');
    x.value = '25';
    x.dispatchEvent(new Event('blur'));
    await settle();
    expect(calls().at(-1)).toMatchObject({ tool: 'set', args: { offset: { x: 25 } } });
  });

  it('switches and places the split', async () => {
    el('#split-on').click();
    await settle();
    expect(calls().at(-1)).toMatchObject({ args: { split: true } });
    await key(el('#split-position [role="slider"]'), 'Home');
    expect(el('#split-value').textContent).toBe('0 %');
    expect(calls().at(-1)).toMatchObject({ args: { split: 0 } });
  });

  it('lists a design choice per breakpoint with what each view shows', async () => {
    const rows = document.querySelectorAll('#views li');
    expect(rows).toHaveLength(3);
    const mobile = select('mobile');
    expect(mobile.getAttribute('aria-label')).toBe('Design for mobile');
    expect(mobile.textContent?.trim()).toBe('mobile.png (393 x 852 px)');
    expect(select('tablet').textContent?.trim()).toBe('Nearest width (automatic)');
    expect(rows[0]?.textContent).toContain('designs/mobile.png, 393 x 852 px, scale 1x');
    expect(rows[2]?.textContent).toContain('The design ../x.png is refused: designs must lie in .rbrowser/designs/.');
    await choose(select('tablet'), 'desktop.png (1440 x 900 px)');
    expect(calls().at(-1)).toMatchObject({ args: { designs: { tablet: 'designs/desktop.png' } } });
    await choose(select('mobile'), 'Nearest width (automatic)');
    expect(calls().at(-1)).toMatchObject({ args: { designs: { mobile: null } } });
  });

  it('follows the posts of the component and asks again when the designs change', async () => {
    await fromHost({ type: 'event', message: { state: { ...ANSWER.state, opacity: 70 }, reload: false } });
    expect(el('#opacity [role="slider"]').getAttribute('aria-valuenow')).toBe('70');
    expect(el('#opacity-value').textContent).toBe('70 %');
    const before = calls().length;
    await fromHost({ type: 'event', message: { state: ANSWER.state, reload: true } });
    // The state again; the designs only when their names change (each listing reads them all).
    expect(calls().slice(before).map((call) => call.tool)).toEqual(['set']);
    expect(calls().slice(before).every((call) => JSON.stringify(call.args) === '{}')).toBe(true);
  });

  it('says when a call failed, in a sentence, never the code', async () => {
    el('#reset').click();
    expect(calls().at(-1)).toMatchObject({ args: { reset: true } });
    await answer('plugins.error.busy', false, { reason: 'no_answer' });
    expect(status().textContent).toBe('Not applied: The plugin is busy.');
    expect(status().dataset.tone).toBe('error');
  });

  it('offers each design by name and size, Nearest width first', async () => {
    expect(texts(await offered(select('mobile')))).toEqual(['Nearest width (automatic)', 'desktop.png (1440 x 900 px)', 'mobile.png (393 x 852 px)']);
    const library = [...document.querySelectorAll<HTMLElement>('#library li')];
    expect(library.map((row) => row.querySelector('[data-name]')?.textContent)).toEqual(['desktop.png', 'mobile.png']);
    expect(library.map((row) => row.querySelector('[data-name]')?.getAttribute('title'))).toEqual(['desktop.png', 'mobile.png']);
    expect(library.map((row) => row.querySelector('[data-size]')?.textContent)).toEqual(['1440 x 900 px', '393 x 852 px']);
    expect(el('#library-title').textContent).toBe('Designs in .rbrowser/designs/');
    expect(document.querySelector('#library-more')).toBeNull();
  });

  it('says how many designs it lists when the folder holds more', async () => {
    await reloaded();
    await reply(isSet, { ...ANSWER, designs: ['desktop.png', 'mobile.png', 'more.png'] });
    await reply(isList, { ...LIBRARY, more: true });
    expect(el('#library-more').textContent).toBe('Only the first 2 designs are listed.');
    await reloaded();
    await reply(isSet, { ...ANSWER, designs: ['desktop.png', 'more.png'] });
    await reply(isList, { ...LIBRARY, designs: [DESKTOP], more: true });
    expect(el('#library-more').textContent).toBe('Only the first design is listed.');
  });

  it('keeps a chosen design that is gone in the choice, marked as missing', async () => {
    const gone = { ...ANSWER, state: { ...ANSWER.state, designs: { mobile: 'designs/mobile.png', tablet: 'designs/old.png' } } };
    await reloaded();
    await reply(isSet, gone);
    const tablet = select('tablet');
    expect(tablet.textContent?.trim()).toBe('old.png (missing)');
    expect(tablet.dataset.missing).toBe('true');
    expect(select('mobile').dataset.missing).toBeUndefined();
    expect(texts(await offered(tablet))).toContain('old.png (missing)');
  });

  it('marks an unusable design in the choice', async () => {
    const broken = { name: 'broken.png', path: 'designs/broken.png', problem: { code: 'plugin.image-overlay.errors.bad_type', params: { path: 'designs/broken.png' } } };
    await reloaded();
    await reply(isSet, { ...ANSWER, designs: ['broken.png', 'desktop.png', 'mobile.png'] });
    await reply(isList, { ...LIBRARY, designs: [broken, DESKTOP, MOBILE] });
    expect(texts(await offered(select('mobile')))).toContain('broken.png (unusable)');
    expect(el('#library').textContent).toContain('The design designs/broken.png is no PNG, JPEG or WebP image.');
  });

  it('shows the preview the component kept, without fetching the design', async () => {
    expect(calls().at(-1)).toMatchObject({ tool: 'designs', args: { thumb: 'aaaaaaaaaaaaaaaa' } });
    await answer({ thumb: 'aaaaaaaaaaaaaaaa', data: 'iVBORw0K' });
    const preview = el<HTMLImageElement>('#library img[data-hash="aaaaaaaaaaaaaaaa"]');
    expect(preview.getAttribute('src')).toBe('data:image/png;base64,iVBORw0K');
    expect(preview.alt).toBe('Preview of desktop.png');
    expect(calls().some((call) => 'chunk' in (call.args ?? {}))).toBe(false);
    // Then the next design, one at a time.
    expect(calls().at(-1)).toMatchObject({ args: { thumb: 'bbbbbbbbbbbbbbbb' } });
    await answer({ thumb: 'bbbbbbbbbbbbbbbb', data: 'bW9iaWxl' });
    // The view that shows mobile.png shows its preview too.
    expect(el('#views li img').getAttribute('src')).toBe('data:image/png;base64,bW9iaWxl');
  });

  it('draws a missing preview once from the design and hands it to the component', async () => {
    vi.spyOn(HTMLImageElement.prototype, 'decode').mockResolvedValue(undefined);
    const draw = vi.fn();
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({ drawImage: draw } as never);
    vi.spyOn(HTMLCanvasElement.prototype, 'toDataURL').mockReturnValue('data:image/png;base64,c21hbGw=');
    await answer({ thumb: 'aaaaaaaaaaaaaaaa', data: null });
    expect(calls().at(-1)).toMatchObject({ args: { path: 'designs/desktop.png', hash: 'aaaaaaaaaaaaaaaa', chunk: 0 } });
    await answer({ index: 0, data: btoa('\x89PNG') });
    expect(draw).toHaveBeenCalledTimes(1);
    const kept = calls().find((call) => call.args?.data !== undefined);
    expect(kept?.args).toEqual({ thumb: 'aaaaaaaaaaaaaaaa', data: 'c21hbGw=' });
    await reply((call) => call === kept, { thumb: 'aaaaaaaaaaaaaaaa', saved: true });
    expect(el('#library img[data-hash="aaaaaaaaaaaaaaaa"]').getAttribute('src')).toBe('data:image/png;base64,c21hbGw=');
  });

  it('shows no preview when it cannot be drawn, and keeps nothing of the design', async () => {
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(null);
    await answer({ thumb: 'aaaaaaaaaaaaaaaa', data: null });
    await answer({ index: 0, data: btoa('\x89PNG') });
    expect(document.querySelector('#library img[data-hash="aaaaaaaaaaaaaaaa"]')).toBeNull();
    expect(document.querySelectorAll('#library li')[0]?.querySelector('[data-pending]')).not.toBeNull();
    expect(calls().some((call) => call.args?.data !== undefined && 'thumb' in (call.args ?? {}))).toBe(false);
    // The next design goes on.
    expect(calls().at(-1)).toMatchObject({ args: { thumb: 'bbbbbbbbbbbbbbbb' } });
  });

  it('explains the empty folder and adds a design through the application', async () => {
    expect(document.querySelector('#designs-none')).toBeNull();
    expect(el('#add-design').textContent).toBe('Add design…');
    expect(el('#add-design').hasAttribute('data-primary')).toBe(true);
    await reloaded();
    await reply(isSet, { ...ANSWER, designs: [] });
    await reply(isList, { ...LIBRARY, designs: [] });
    const empty = el('#designs-none');
    expect(empty.textContent).toContain(
      'No designs yet. Add a PNG, JPEG or WebP export of your design; it is copied into .rbrowser/designs/ so the team shares it.',
    );
    // One way to add at a time: the empty state's button, not the header's as well.
    expect(document.querySelector('#add-design')).toBeNull();
    const add = el<HTMLButtonElement>('#add-design-empty');
    expect(add.textContent?.trim()).toBe('Add design…');
    add.click();
    await settle();
    expect(calls().at(-1)).toMatchObject({ command: 'add_design', args: {} });
    expect(calls().at(-1)?.tool).toBeUndefined();
    expect(status().textContent).toBe('Choose the image in the file dialog.');
    // Busy until the application answers - a design, a refusal or a cancelled dialog.
    expect(add.disabled).toBe(true);
    expect(add.getAttribute('aria-busy')).toBe('true');
    await fromHost({ type: 'event', message: { hostNotice: { text: 'No design added: the file dialog was cancelled.', failed: false } } });
    expect(status().textContent).toBe('No design added: the file dialog was cancelled.');
    expect(add.disabled).toBe(false);
    expect(add.hasAttribute('aria-busy')).toBe(false);
  });

  it('says a failed Add design in a sentence and ends the busy button', async () => {
    el('#add-design').click();
    await settle();
    expect((el('#add-design') as HTMLButtonElement).disabled).toBe(true);
    await answer('plugins.error.busy', false);
    expect(status().dataset.tone).toBe('error');
    expect(status().textContent).toBe('Not applied: The plugin is busy.');
    expect((el('#add-design') as HTMLButtonElement).disabled).toBe(false);
  });

  it('shows what the application says about the design it added or refused', async () => {
    await fromHost({ type: 'event', message: { hostNotice: { text: 'Design home.png added.', failed: false } } });
    expect(status().textContent).toBe('Design home.png added.');
    expect(status().dataset.tone).not.toBe('error');
    await fromHost({ type: 'event', message: { hostNotice: { text: 'Trust this workspace first.', failed: true } } });
    expect(status().textContent).toBe('Trust this workspace first.');
    expect(status().dataset.tone).toBe('error');
  });

  it('says why a design cannot be added without a trusted project', async () => {
    expect(document.querySelector('#add-untrusted')).toBeNull();
    // The application's answer to Add design is when trust may have changed: the panel asks again.
    await fromHost({ type: 'event', message: { hostNotice: { text: 'Trust this workspace first.', failed: true } } });
    expect(calls().at(-1)).toMatchObject({ tool: 'designs', args: {} });
    await reply(isList, { ...LIBRARY, trusted: false });
    expect(el('#add-untrusted').textContent).toContain('trusted workspace');
  });

  it('takes a dropped file to the button instead of letting the frame open it', async () => {
    const over = new Event('dragover', { bubbles: true, cancelable: true });
    document.body.dispatchEvent(over);
    expect(over.defaultPrevented).toBe(true);
    const drop = new Event('drop', { bubbles: true, cancelable: true });
    document.body.dispatchEvent(drop);
    expect(drop.defaultPrevented).toBe(true);
    await settle();
    expect(status().textContent).toBe('Dropping a file here does not add it: use Add design…, which opens the file dialog.');
  });

  it("takes the app's theme from its settings", async () => {
    await fromHost({ type: 'settings', strings: STRINGS(), settings: { 'ui.theme': 'dark' } });
    expect(document.documentElement.classList.contains('dark')).toBe(true);
  });

  it('ignores messages from anyone but the panels window', async () => {
    const before = calls().length;
    window.dispatchEvent(new MessageEvent('message', { data: { type: 'event', message: { state: { ...ANSWER.state, opacity: 5 } } }, source: null }));
    await settle();
    expect(el('#opacity-value').textContent).toBe('50 %');
    expect(calls().length).toBe(before);
  });
});
