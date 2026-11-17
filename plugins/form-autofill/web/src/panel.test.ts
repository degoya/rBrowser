// The panel in a window of its own: the bridge messages it sends and how it renders the plugin's
// answers. The panels window is played by spying on postMessage and dispatching `message` events
// from window.parent (the test window itself). On Nuxt UI since RB-080-86.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { mountPanel } from '../../../web-common/panel-ui/src/mount';
import App from './panel/App.vue';
import type { AutofillField, AutofillResult } from './panel/model';
import { pluginFile } from './support';

type Posted = { type?: string; id?: number; tool?: string; command?: string; args?: Record<string, unknown> };
let posted: Posted[] = [];
let unmount: () => void = () => undefined;

/** The plugin's texts flattened, as the host hands them to a panel. */
function flatten(value: unknown, prefix: string, out: Record<string, string>): Record<string, string> {
  if (typeof value === 'string') out[prefix] = value;
  else if (value && typeof value === 'object') {
    for (const [key, inner] of Object.entries(value)) flatten(inner, prefix ? `${prefix}.${key}` : key, out);
  }
  return out;
}
const STRINGS = flatten(JSON.parse(pluginFile('locales/en.json')), '', {});
const SETTINGS = {
  'form-autofill.profiles': [{ name: 'testuser', values: { email: 'qa@example.com' } }],
  'form-autofill.activeProfile': 'testuser',
  'form-autofill.invalidStrategy': 'bounds',
};

const field = (key: string, extra: Partial<AutofillField> = {}): AutofillField => ({
  key, label: key, kind: 'text', value: 'v', violation: null, profile: false, skip: '', status: 'done', valid: true, errors: [], ...extra,
});

const RESULT: AutofillResult = {
  mode: 'invalid', seed: 42, strategy: 'mixed', profile: null,
  views: [
    { view: 1, label: 'Mobile', breakpoint: 'mobile', engine: 'chromium', status: 'filled', formValid: false, blockedSubmits: 0,
      forms: [{ selector: '#register', name: 'Register' }],
      fields: [
        field('input:email:email', { label: 'E-mail', kind: 'email', value: 'qa.example.com', violation: 'format', valid: false, errors: ['typeMismatch'], required: true, form: 0 }),
        field('input:file:avatar', { label: 'Picture', value: '', skip: 'file', status: null, valid: null, form: 0 }),
      ] },
    { view: 4, label: 'Firefox', breakpoint: 'fx', engine: 'firefox', status: 'unsupported', fields: [] },
  ],
};

/** Two views that differ in one field, on a page with two forms. */
const TWO_FORMS: AutofillResult = {
  mode: 'valid', seed: 706559, strategy: null, profile: 'testuser',
  views: [1, 2].map((id) => ({
    view: id, label: id === 1 ? 'md' : 'lg', breakpoint: '', engine: 'chromium', status: 'filled', formValid: true, blockedSubmits: id,
    forms: [{ selector: '#login', name: 'Sign in' }, { selector: 'body > form:nth-child(2)', name: '' }],
    fields: [
      field('user', { label: 'User name with a very long label that never wraps', value: 'linda', form: 0 }),
      field('mail', { label: 'Mail', kind: 'email', value: id === 1 ? 'linda@example.com' : 'other@example.com', form: 1 }),
    ],
  })),
};

const $ = (selector: string): HTMLElement | null => document.querySelector<HTMLElement>(selector);
const settle = async (): Promise<void> => {
  await new Promise((resolve) => setTimeout(resolve, 0));
  await nextTick();
};
async function fromHost(data: unknown): Promise<void> {
  window.dispatchEvent(new MessageEvent('message', { data, source: window }));
  await settle();
}
// The fill's calls; the panel also asks for the stored presets and a pending capture once its
// texts are in (panel-capture.test.ts).
const calls = (): Posted[] => posted.filter((m) => m.type === 'call' && m.tool !== 'presets' && m.tool !== 'capture');

beforeEach(async () => {
  posted = [];
  document.body.innerHTML = '<div id="app"></div>';
  vi.spyOn(window, 'postMessage').mockImplementation((message: unknown) => {
    posted.push(message as Posted);
  });
  unmount = mountPanel(App, { prefix: 'plugin.form-autofill.', name: 'form autofill panel' }).unmount;
  await fromHost({ type: 'settings', strings: STRINGS, settings: SETTINGS });
});

afterEach(() => {
  unmount();
  vi.restoreAllMocks();
});

describe('the autofill panel', () => {
  it('asks for its texts and labels every control', async () => {
    expect(posted[0]).toEqual({ type: 'settings' });
    expect($('#title')?.textContent).toBe('Form autofill');
    expect($('#fill-valid')?.textContent).toBe('Fill valid');
    expect($('#fill-valid')?.hasAttribute('data-primary')).toBe(true);
    expect($('#fill-invalid')?.textContent).toBe('Fill invalid');
    const options = $('#options') as HTMLButtonElement;
    expect(options.getAttribute('aria-label')).toBe('Options');
    expect(options.textContent).toBe('');
    expect(options.getAttribute('aria-expanded')).toBe('false');
    options.click();
    await settle();
    expect(options.getAttribute('aria-expanded')).toBe('true');
    for (const id of ['strategy', 'profile', 'selector', 'seed']) {
      expect(document.querySelector(`label[for="${id}"]`)?.textContent, id).toBeTruthy();
    }
    expect($('#strategy')?.textContent).toContain('Out of bounds');
    expect($('#profile')?.textContent).toContain('testuser');
    expect($('#selector')?.textContent).toContain('All forms');
    expect(($('#seed') as HTMLInputElement).placeholder).toBe('From the settings');
    expect($('#new-seed')?.getAttribute('aria-label')).toBe('New seed');
    expect($('#empty')?.textContent).toContain('No fill yet.');
  });

  it('fills through the tool and shows the result per field', async () => {
    ($('#fill-invalid') as HTMLButtonElement).click();
    await nextTick();
    const call = calls()[0];
    expect(call).toMatchObject({ tool: 'fill', args: { mode: 'invalid', strategy: 'bounds', profile: 'testuser' } });
    expect(call?.args).not.toHaveProperty('selector');
    expect(($('#fill-valid') as HTMLButtonElement).disabled).toBe(true);
    expect($('#status')?.dataset.tone).toBe('busy');
    await fromHost({ type: 'result', id: call?.id, ok: true, value: RESULT });
    expect(($('#fill-valid') as HTMLButtonElement).disabled).toBe(false);
    expect($('#status')?.textContent).toBe('Filled 1 field with invalid values in 1 view, seed 42. Firefox: Firefox, not filled.');
    expect($('#status')?.dataset.tone).toBe('ok');
    const rows = [...document.querySelectorAll('[data-fields] tbody tr')];
    expect(rows).toHaveLength(1);
    const row = rows[0] as HTMLElement;
    expect(row.querySelector('[data-name]')?.textContent).toBe('E-mail*required');
    expect(row.querySelector('[data-kind]')?.textContent).toBe('email');
    const value = row.querySelector('[data-value]') as HTMLElement;
    expect(value.textContent).toBe('qa.example.com');
    expect(value.title).toBe('qa.example.com');
    expect(value.className).toContain('truncate');
    expect([...row.querySelectorAll('[data-mark]')].map((mark) => mark.textContent)).toEqual(['invalid', 'broken: format']);
    expect(row.querySelector('[data-differs]')).toBeNull();
    expect($('[data-skipped]')?.textContent).toContain('1 field skipped');
    expect($('[data-skipped-field]')?.textContent).toContain('Picture');
    expect($('[data-skipped-field] [data-reason]')?.textContent).toBe('file input');
    expect($('[data-fact="mode"]')?.textContent).toBe('invalid values');
    expect($('[data-fact="strategy"]')?.textContent).toBe('Mixed');
    expect($('[data-fact="form"]')?.textContent).toBe('Form invalid');
    expect($('#seed-used')?.textContent).toBe('Seed 42');
    // One form: no card per form.
    expect(document.querySelectorAll('[data-form]')).toHaveLength(0);
  });

  it('opens a field that differs to its value per view, and shows a card per form', async () => {
    await fromHost({ type: 'event', message: { type: 'filled', result: TWO_FORMS } });
    const cards = [...document.querySelectorAll<HTMLElement>('[data-form]')];
    expect(cards.map((card) => card.querySelector('h2')?.textContent)).toEqual(['Sign in', 'Form 2']);
    expect(cards[1]?.querySelector('code')?.textContent).toBe('body > form:nth-child(2)');
    const user = cards[0]?.querySelector('[data-name] span') as HTMLElement;
    expect(user.className).toContain('truncate');
    expect(user.title).toBe('User name with a very long label that never wraps');
    const differs = cards[1]?.querySelector('[data-differs]') as HTMLButtonElement;
    expect(differs.textContent).toBe('differs in 1 view');
    expect(differs.getAttribute('aria-expanded')).toBe('false');
    expect(cards[1]?.querySelector('[data-per-view]')).toBeNull();
    differs.click();
    await settle();
    expect(differs.getAttribute('aria-expanded')).toBe('true');
    const perView = [...(cards[1]?.querySelectorAll('[data-per-view] li') ?? [])].map((li) => li.textContent);
    expect(perView).toEqual(['mdlinda@example.com', 'lgother@example.com']);
    // New settings (the theme, the texts: a copy, as a posted message is) keep the row open.
    await fromHost({ type: 'settings', strings: { ...STRINGS }, settings: { ...SETTINGS, 'ui.theme': 'dark' } });
    expect(document.querySelectorAll('[data-per-view] li')).toHaveLength(2);
    expect($('[data-fact="profile"]')?.textContent).toBe('Profile: testuser');
    expect($('[data-fact="blocked"]')?.textContent).toBe('3 submits cancelled');
    expect($('#status')?.textContent).toBe('Filled 2 fields with valid values in 2 views, seed 706559.');
  });

  it('offers the forms found as the choice of a form and sends the one chosen', async () => {
    await fromHost({ type: 'event', message: { type: 'filled', result: TWO_FORMS } });
    ($('#options') as HTMLButtonElement).click();
    await settle();
    ($('#selector') as HTMLElement).click();
    await settle();
    const items = [...document.querySelectorAll<HTMLElement>('[role="option"]')];
    expect(items.map((item) => item.textContent)).toEqual(['All forms', 'Sign in#login', 'Form 2body > form:nth-child(2)']);
    items[1]?.click();
    await settle();
    ($('#fill-valid') as HTMLButtonElement).click();
    await nextTick();
    expect(calls()[0]?.args).toEqual({ mode: 'valid', selector: '#login', profile: 'testuser' });
  });

  it('draws a new seed and sends it', async () => {
    vi.spyOn(Math, 'random').mockReturnValue(0.5);
    ($('#options') as HTMLButtonElement).click();
    await settle();
    ($('#new-seed') as HTMLButtonElement).click();
    await settle();
    expect(($('#seed') as HTMLInputElement).value).toBe('500000');
    ($('#fill-valid') as HTMLButtonElement).click();
    await nextTick();
    expect(calls()[0]?.args).toMatchObject({ mode: 'valid', seed: 500000 });
  });

  it('copies the seed of the fill', async () => {
    const writeText = vi.fn(async () => undefined);
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
    await fromHost({ type: 'event', message: { type: 'filled', result: RESULT } });
    const copy = $('#copy-seed') as HTMLButtonElement;
    expect(copy.getAttribute('aria-label')).toBe('Copy the seed');
    copy.click();
    await settle();
    expect(writeText).toHaveBeenCalledWith('42');
    expect(copy.getAttribute('aria-label')).toBe('Seed copied');
  });

  it('shows a failure in words', async () => {
    ($('#fill-valid') as HTMLButtonElement).click();
    await fromHost({ type: 'result', id: calls()[0]?.id, ok: false, error: 'plugin.form-autofill.errors.profile_unknown' });
    expect($('#status')?.textContent).toContain('The fill failed: The autofill profile is not in the settings');
    expect($('#status')?.dataset.tone).toBe('error');
    ($('#fill-valid') as HTMLButtonElement).click();
    await fromHost({ type: 'result', id: calls()[1]?.id, ok: false, error: 'plugin.form-autofill.errors.profile_unknown', params: { name: 'ghost' } });
    expect($('#status')?.textContent).toContain('The autofill profile ghost is not in the settings');
  });

  it('shows a fill from a shortcut and ignores messages from elsewhere', async () => {
    window.dispatchEvent(new MessageEvent('message', { data: { type: 'event', message: { type: 'filled', result: RESULT } }, source: null }));
    await settle();
    expect(document.querySelectorAll('[data-fields] tbody tr')).toHaveLength(0);
    expect($('#empty')).not.toBeNull();
    await fromHost({ type: 'event', message: { type: 'filled', result: RESULT } });
    expect(document.querySelectorAll('[data-fields] tbody tr')).toHaveLength(1);
    expect($('#empty')).toBeNull();
    await fromHost({ type: 'event', message: { type: 'filled', tooLarge: true, seed: 9, mode: 'valid' } });
    expect($('#status')?.textContent).toContain('seed 9');
  });

  it("takes the app's theme from its settings", async () => {
    await fromHost({ type: 'settings', strings: STRINGS, settings: { ...SETTINGS, 'ui.theme': 'dark' } });
    expect(document.documentElement.classList.contains('dark')).toBe(true);
  });
});
