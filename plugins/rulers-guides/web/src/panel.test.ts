// The rulers-guides panel in a window of its own: the bridge messages it sends and how it renders
// the plugin's answers. The panels window is played by spying on postMessage and dispatching
// `message` events from window.parent (the test window itself). On Nuxt UI since RB-080-86.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { mountPanel } from '../../../web-common/panel-ui/src/mount';
import App from './panel/App.vue';
import en from '../../locales/en.json';

type Sent = { type: string; id?: number; tool?: string; args?: Record<string, unknown> };

const STATE = {
  visible: { rulers: true, columns: false, baseline: false },
  guides: [
    { id: 1, axis: 'x', unit: 'percent', value: 50 },
    { id: 2, axis: 'y', unit: 'px', value: 320 },
  ],
  columns: { '*': { preset: 'bootstrap' }, mobile: { columns: 4, gutter: 16, margin: 8 } },
  baseline: { lineHeight: 8, offset: 0 },
};

// As the component answers them (src/columns.rs `templates`).
const TEMPLATES = {
  '': { columns: 12, gutter: 24, margin: 0, maxWidth: null },
  bootstrap: { columns: 12, gutter: 24, margin: 12, maxWidth: { min: 540, max: 1320 } },
  tailwind: { columns: 12, gutter: 24, margin: 16, maxWidth: { min: 640, max: 1536 } },
};

let sent: Sent[] = [];
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

/** Answers the last call with `value`, or fails it with the code `value`. */
async function answer(value: unknown, ok = true): Promise<void> {
  const call = calls().at(-1);
  await fromHost(ok ? { type: 'result', id: call?.id, ok, value } : { type: 'result', id: call?.id, ok, error: value });
}

function el<T extends HTMLElement>(selector: string): T {
  const found = document.querySelector(selector);
  if (!found) throw new Error(`no ${selector}`);
  return found as T;
}

const input = (selector: string): HTMLInputElement => el<HTMLInputElement>(selector);

async function key(target: HTMLElement, name: string): Promise<KeyboardEvent> {
  const event = new KeyboardEvent('keydown', { key: name, bubbles: true, cancelable: true });
  target.dispatchEvent(event);
  await settle();
  return event;
}

/** Types `value` into a field and leaves it: the browser's `change`. */
async function type(selector: string, value: string): Promise<void> {
  input(selector).value = value;
  input(selector).dispatchEvent(new Event('change', { bubbles: true }));
  await settle();
}

/** The entries a select offers, opened by keyboard. */
async function offered(trigger: HTMLElement): Promise<HTMLElement[]> {
  await key(trigger, 'Enter');
  return [...document.querySelectorAll<HTMLElement>('[role="option"]')];
}

const texts = (nodes: HTMLElement[]): string[] => nodes.map((node) => (node.textContent ?? '').trim());

/** Opens the select and picks the entry that reads `label` - by keyboard, Enter on the entry. */
async function choose(trigger: HTMLElement, label: string): Promise<void> {
  const option = (await offered(trigger)).find((one) => (one.textContent ?? '').trim() === label);
  if (!option) throw new Error(`no entry ${label}`);
  option.focus();
  await key(option, 'Enter');
}

const status = (): HTMLElement => el('#status');

beforeEach(async () => {
  sent = [];
  document.body.innerHTML = '<div id="app"></div>';
  vi.spyOn(window, 'postMessage').mockImplementation((message: unknown) => {
    sent.push(message as Sent);
  });
  unmount = mountPanel(App, { prefix: 'plugin.rulers-guides.', name: 'rulers-guides panel' }).unmount;
  await fromHost({ type: 'settings', strings: flatten(en, '', {}), settings: {} });
  await answer({ state: STATE, breakpoints: ['desktop', 'mobile'], templates: TEMPLATES });
});

afterEach(() => {
  unmount();
  vi.restoreAllMocks();
});

describe('the panel', () => {
  it('asks for its texts and the state, and shows them', () => {
    expect(sent[0]).toEqual({ type: 'settings' });
    expect(calls()[0]).toMatchObject({ tool: 'set', args: {} });
    expect(el('#title').textContent).toBe('Rulers and guides');
    expect(el('#show-rulers').getAttribute('aria-checked')).toBe('true');
    expect(el('#show-columns').getAttribute('aria-checked')).toBe('false');
    expect(document.querySelectorAll('#guides li')).toHaveLength(2);
    expect(document.querySelector('#guides-empty')).toBeNull();
  });

  it('labels every control for the keyboard and the screen reader', () => {
    for (const control of document.querySelectorAll('input, select, button')) {
      const id = control.getAttribute('id');
      const labelled =
        control.getAttribute('aria-label') ||
        (id && document.querySelector(`label[for="${id}"]`)) ||
        control.closest('label') ||
        (control.textContent ?? '').trim();
      expect(labelled, control.outerHTML).toBeTruthy();
    }
    expect(el('#guides li [data-key="unit-0"]').getAttribute('aria-label')).toBe('Guide 1, Vertical: Measured');
    expect(el('#guides li [data-key="value-0"]').getAttribute('aria-label')).toBe('Guide 1, Vertical: Position');
    const remove = el('[data-key="remove-0"]');
    expect(remove.getAttribute('aria-label')).toBe('Remove guide 1');
    expect(remove.textContent).toBe('');
  });

  it('switches a layer', async () => {
    el('#show-columns').click();
    await settle();
    expect(calls().at(-1)?.args).toEqual({ visible: { columns: true } });
    await answer({ state: { ...STATE, visible: { ...STATE.visible, columns: true } }, saved: true });
    expect(status().textContent).toBe('Saved.');
    expect(el('#show-columns').getAttribute('aria-checked')).toBe('true');
  });

  it('sets a guide by typing its value', async () => {
    await type('#guides li input[type="number"]', '25');
    expect(calls().at(-1)?.args).toEqual({
      guides: [
        { axis: 'x', unit: 'percent', value: 25 },
        { axis: 'y', unit: 'px', value: 320 },
      ],
    });
  });

  it('measures a vertical guide from another edge', async () => {
    expect(texts(await offered(el('[data-key="unit-0"]')))).toEqual(['px from the left', 'px from the right', '% of the width']);
    await choose(el('[data-key="unit-0"]'), 'px from the right');
    expect(calls().at(-1)?.args).toEqual({
      guides: [
        { axis: 'x', unit: 'px-right', value: 50 },
        { axis: 'y', unit: 'px', value: 320 },
      ],
    });
  });

  it('adds and removes guides', async () => {
    el('#add-y').click();
    await settle();
    expect((calls().at(-1)?.args?.guides as unknown[]).at(-1)).toEqual({ axis: 'y', unit: 'px', value: 100 });
    el('[data-key="remove-0"]').click();
    await settle();
    expect(calls().at(-1)?.args).toEqual({ guides: [{ axis: 'y', unit: 'px', value: 320 }] });
  });

  it('a horizontal guide counts px from the top only', () => {
    const select = el<HTMLButtonElement>('[data-key="unit-1"]');
    expect(select.disabled).toBe(true);
    expect(select.textContent?.trim()).toBe('px from the top');
  });

  it('shows and applies the column setup of the chosen breakpoint', async () => {
    const breakpoint = el('#breakpoint');
    expect(texts(await offered(breakpoint))).toEqual(['desktop', 'mobile', 'Every other breakpoint (*)']);
    await choose(el('#breakpoint'), 'mobile');
    expect(input('#columns').value).toBe('4');
    expect(input('#max-width').value).toBe('');
    await type('#max-width', '1200');
    expect(calls().at(-1)?.args).toEqual({
      columns: { mobile: { columns: 4, gutter: 16, margin: 8, maxWidth: 1200 } },
      visible: { columns: true },
    });
    el('#columns-off').click();
    await settle();
    expect(calls().at(-1)?.args).toEqual({ columns: { mobile: null } });
  });

  it('sets the baseline grid and resets to the shared settings', async () => {
    input('#line-height').value = '24';
    await type('#offset', '4');
    expect(calls().at(-1)?.args).toEqual({ baseline: { lineHeight: 24, offset: 4 }, visible: { baseline: true } });
    el('#reset').click();
    await settle();
    expect(calls().at(-1)?.args).toEqual({ reset: true });
  });

  // RB-080-66: the panel's frame is sandboxed without `allow-forms`; Chromium blocks a form's
  // submission there before its `submit` event ("Blocked form submission ... 'allow-forms'"),
  // so Apply did nothing. No form: Apply is a button, Enter in a field applies its group.
  it('has no form the sandbox would block and no Apply: a change or Enter applies its group', async () => {
    expect(document.querySelector('form')).toBeNull();
    expect(document.querySelector('button[type="submit"], button:not([type])')).toBeNull();
    input('#line-height').value = '12';
    input('#offset').value = '';
    await key(el('#line-height'), 'Enter');
    expect(calls().at(-1)?.args).toEqual({ baseline: { lineHeight: 12, offset: 0 }, visible: { baseline: true } });
    input('#gutter').value = '24';
    await key(el('#gutter'), 'Enter');
    expect(calls().at(-1)?.args).toMatchObject({ columns: { desktop: { gutter: 24 } } });
  });

  it('applies nothing while a field of the group is invalid', async () => {
    const count = calls().length;
    await type('#line-height', '');
    await type('#columns', '99');
    expect(calls().length).toBe(count);
  });

  // RB-090-58: a field per colour; a typed one applies on its own, as the person's colour.
  it('shows the colours and applies a typed one', async () => {
    expect(document.querySelector('#colors')).toBeNull();
    const colors = { ruler: '#f6f7f9f0', ticks: '#4a4f5c', guide: '#00a2ff', columnFill: '#ff40811f', columnEdge: '#ff408100', baseline: '#00a2ff59' };
    await fromHost({ type: 'event', message: { state: { ...STATE, colors } } });
    expect(el('#colors-title').textContent).toBe('Colours');
    expect(input('#color-baseline').value).toBe('#00a2ff59');
    expect(input('#color-guide').getAttribute('aria-label')).toBe('Guides, hex (#rrggbb or #rrggbbaa)');
    expect(el('#color-guide-pick').getAttribute('aria-label')).toBe('Pick the colour: Guides');
    await type('#color-guide', '#123456');
    expect(calls().at(-1)?.args).toEqual({ colors: { guide: '#123456' } });
  });

  it('follows a change made in a view', async () => {
    await fromHost({ type: 'event', message: { state: { ...STATE, guides: [] } } });
    expect(document.querySelectorAll('#guides li')).toHaveLength(0);
    expect(el('#guides-empty').textContent).toBe('No guides yet. Drag one out of a ruler or add one here.');
  });

  it('says when a change is not saved or failed', async () => {
    el('#reset').click();
    await answer({ state: STATE, saved: false });
    expect(status().textContent).toContain('not saved');
    el('#reset').click();
    await answer('plugin.rulers-guides.errors.bad_args', false);
    expect(status().textContent).toBe('Not applied: Rulers and guides got a value it cannot use.');
    expect(status().dataset.tone).toBe('error');
  });

  // RB-080-66 (owner, 20.png): "none of the settings seem to have any effect".
  it('says why a change failed in a sentence, never as a code', async () => {
    el('#reset').click();
    await answer('plugins.error.busy', false);
    expect(status().textContent).toBe('Not applied: rBrowser is still busy with earlier requests of the rulers. Try again in a moment.');
    el('#reset').click();
    await answer('plugins.error.something_new', false);
    expect(status().textContent).not.toContain('plugins.error');
    // A failure is read out at once.
    expect(status().getAttribute('aria-live')).toBe('assertive');
  });

  // RB-094-21: "Resume them in Settings > Plugins" links there; another failure links nowhere.
  it('links a paused plugin to Settings > Plugins', async () => {
    el('#reset').click();
    await answer('plugins.error.suspended', false);
    el('#status [data-testid="setting-link-plugins"]').click();
    expect(sent).toContainEqual({ target: 'host', action: 'open_settings', key: 'plugins' });
    el('#reset').click();
    await answer('plugins.error.busy', false);
    expect(document.querySelector('#status [data-testid="setting-link-plugins"]')).toBeNull();
  });

  it('says a failed first read in a sentence too', async () => {
    unmount();
    sent = [];
    document.body.innerHTML = '<div id="app"></div>';
    unmount = mountPanel(App, { prefix: 'plugin.rulers-guides.', name: 'rulers-guides panel' }).unmount;
    await fromHost({ type: 'settings', strings: flatten(en, '', {}), settings: {} });
    await answer('plugins.error.timed_out', false);
    expect(status().textContent).toBe('Not applied: The rulers took too long and were stopped. Try again; the rBrowser log has the details.');
  });

  it("shows the template's values as the placeholders of the empty fields", async () => {
    await choose(el('#breakpoint'), 'Every other breakpoint (*)');
    expect(el('#preset').textContent?.trim()).toBe('Bootstrap');
    expect(input('#columns').placeholder).toBe('12');
    expect(input('#gutter').placeholder).toBe('24');
    expect(input('#margin').placeholder).toBe('12');
    expect(input('#max-width').placeholder).toBe('540-1320');
    // Without a template: the defaults, the full width.
    await choose(el('#breakpoint'), 'mobile');
    expect(input('#margin').placeholder).toBe('0');
    expect(input('#max-width').placeholder).toBe('full width');
  });

  it('applies every change of a column field at once and shows the overlay', async () => {
    const count = calls().length;
    await choose(el('#breakpoint'), 'mobile');
    // Choosing which breakpoint to edit changes nothing.
    expect(calls().length).toBe(count);
    await type('#gutter', '20');
    expect(calls().at(-1)?.args).toEqual({
      columns: { mobile: { columns: 4, gutter: 20, margin: 8 } },
      visible: { columns: true },
    });
    await choose(el('#preset'), 'Tailwind');
    expect(calls().at(-1)?.args).toMatchObject({ columns: { mobile: { preset: 'tailwind' } } });
    expect(input('#margin').placeholder).toBe('16');
    await choose(el('#align'), 'Centre');
    expect(calls().at(-1)?.args).toMatchObject({ columns: { mobile: { preset: 'tailwind', align: 'center' } } });
  });

  // Review of RB-080-66, 6: Enter applies, the blur after it must not apply the same again.
  it('applies a value once: Enter and the change of the blur after it make one call', async () => {
    const count = calls().length;
    input('#offset').value = '6';
    await key(el('#offset'), 'Enter');
    el('#offset').dispatchEvent(new Event('change', { bubbles: true }));
    await settle();
    expect(calls().length).toBe(count + 1);
    await answer({ state: { ...STATE, visible: { ...STATE.visible, baseline: true }, baseline: { lineHeight: 8, offset: 6 } }, saved: true });
    el('#offset').dispatchEvent(new Event('change', { bubbles: true }));
    await settle();
    expect(calls().length).toBe(count + 1);
    // Another value applies again.
    await type('#offset', '7');
    expect(calls().length).toBe(count + 2);
  });

  it('applies a baseline change at once and shows the grid', async () => {
    await type('#offset', '6');
    expect(calls().at(-1)?.args).toEqual({ baseline: { lineHeight: 8, offset: 6 }, visible: { baseline: true } });
  });

  it("takes the app's theme from its settings", async () => {
    await fromHost({ type: 'settings', strings: flatten(en, '', {}), settings: { 'ui.theme': 'dark' } });
    expect(document.documentElement.classList.contains('dark')).toBe(true);
  });

  it('ignores messages from anyone but the panels window', async () => {
    window.dispatchEvent(new MessageEvent('message', { data: { type: 'event', message: { state: { ...STATE, guides: [] } } } }));
    await settle();
    expect(document.querySelectorAll('#guides li')).toHaveLength(2);
  });
});
