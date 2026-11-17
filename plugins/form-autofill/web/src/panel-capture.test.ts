// "Take form as preset" in the panel (RB-090-70): the calls it makes, the dialog it opens - name,
// a box per field, the fields not taken with their reason - and what Save sends. The panels
// window is played as in panel.test.ts.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { mountPanel } from '../../../web-common/panel-ui/src/mount';
import App from './panel/App.vue';
import type { Capture } from './panel/capture';
import { pluginFile } from './support';

type Posted = { type?: string; id?: number; tool?: string; args?: Record<string, unknown> };
let posted: Posted[] = [];
let unmount: () => void = () => undefined;

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
  'form-autofill.activeProfile': 'Login',
};

const CAPTURE: Capture = {
  view: 2, label: 'Desktop', url: 'https://shop.example.com/register', scope: 'focused',
  form: { selector: '#signup', name: 'Sign up' }, name: 'Sign up', truncated: false, available: true,
  presets: ['Login'], profiles: ['testuser'],
  fields: [
    { locator: 'email', label: 'E-mail', kind: 'email', control: 'email', form: 0, selected: true, value: 'linda@example.com' },
    { locator: 'nick', label: 'Nickname', kind: 'text', control: 'text', form: 0, selected: false, value: '' },
    { locator: 'terms', label: 'Terms', kind: 'checkbox', control: 'checkbox', form: 0, selected: true, value: 'true' },
    { locator: 'pw', label: 'Password', kind: 'password', control: 'password', form: 0, selected: false, excluded: 'password' },
    { locator: 'avatar', label: 'Picture', kind: 'text', control: 'file', form: 0, selected: false, excluded: 'file' },
  ],
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
const callsOf = (tool: string): Posted[] => posted.filter((m) => m.type === 'call' && m.tool === tool);
const answer = (call: Posted | undefined, value: unknown): Promise<void> => fromHost({ type: 'result', id: call?.id, ok: true, value });

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

/** Opens the dialog with CAPTURE through the header action. */
async function openDialog(): Promise<void> {
  ($('#take-form') as HTMLButtonElement).click();
  await settle();
  await answer(callsOf('capture').at(-1), CAPTURE);
  await settle();
}

describe('take form as preset', () => {
  it('asks once for the stored presets and a pending capture, and offers the presets as profiles', async () => {
    expect(callsOf('presets')).toHaveLength(1);
    expect(callsOf('capture')).toEqual([expect.objectContaining({ args: { pending: true } })]);
    await answer(callsOf('capture')[0], { pending: false });
    expect($('[data-capture]')).toBeNull();
    await answer(callsOf('presets')[0], { presets: [{ name: 'Login', fields: 2 }], available: true });
    ($('#options') as HTMLButtonElement).click();
    await settle();
    // The settings name the preset: it is chosen once the list is in.
    expect($('#profile')?.textContent).toContain('Login');
    ($('#fill-valid') as HTMLButtonElement).click();
    await nextTick();
    expect(callsOf('fill')[0]?.args).toMatchObject({ mode: 'valid', profile: 'Login' });
  });

  it('reads the active view from the header action and opens the dialog with a box per field', async () => {
    const action = $('#take-form') as HTMLButtonElement;
    expect(action.getAttribute('aria-label')).toBe('Take form as preset');
    await openDialog();
    expect(callsOf('capture').at(-1)?.args).toEqual({});
    expect($('[data-capture]')).not.toBeNull();
    expect(document.body.textContent).toContain('New preset from a form');
    expect(document.body.textContent).toContain('Desktop: Sign up');
    expect(($('#preset-name') as HTMLInputElement).value).toBe('Sign up');
    const boxes = [...document.querySelectorAll<HTMLElement>('[data-capture-fields] li')];
    expect(boxes.map((box) => box.textContent)).toEqual(['E-maillinda@example.com', 'Nicknameempty', 'Termstrue']);
    expect(($('#take-0') as HTMLElement).getAttribute('aria-checked')).toBe('true');
    expect(($('#take-1') as HTMLElement).getAttribute('aria-checked')).toBe('false');
    const excluded = [...document.querySelectorAll<HTMLElement>('[data-excluded-field]')].map((row) => row.textContent);
    expect(excluded).toEqual(['Passwordpassword field', 'Picturefile input']);
    expect(document.body.textContent).toContain('2 fields not taken');
    expect(document.body.textContent).toContain('2 fields chosen');
  });

  it('saves the fields ticked under the name typed and says so', async () => {
    await openDialog();
    ($('#take-2') as HTMLElement).click();
    await settle();
    const name = $('#preset-name') as HTMLInputElement;
    name.value = 'Checkout';
    name.dispatchEvent(new Event('input'));
    await settle();
    ($('#save-preset') as HTMLButtonElement).click();
    await settle();
    const save = callsOf('save')[0];
    expect(save?.args).toEqual({ name: 'Checkout', values: { email: 'linda@example.com' }, replace: false });
    await answer(save, { saved: 'Checkout', replaced: false, fields: 1, dropped: [] });
    expect($('[data-capture]')).toBeNull();
    expect($('#status')?.textContent).toBe('Saved the preset Checkout with 1 field.');
    expect($('#status')?.dataset.tone).toBe('ok');
  });

  it('warns that a stored name is replaced and refuses a settings profile name', async () => {
    await openDialog();
    const name = $('#preset-name') as HTMLInputElement;
    name.value = 'Login';
    name.dispatchEvent(new Event('input'));
    await settle();
    expect($('#name-replaces')?.textContent).toContain('Saving replaces the preset Login.');
    expect(($('#save-preset') as HTMLButtonElement).disabled).toBe(false);
    name.value = 'testuser';
    name.dispatchEvent(new Event('input'));
    await settle();
    expect($('#name-in-settings')?.textContent).toContain('testuser is a profile in the settings');
    expect(($('#save-preset') as HTMLButtonElement).disabled).toBe(true);
  });

  it('cannot save without a project and keeps a refusal in the dialog', async () => {
    ($('#take-form') as HTMLButtonElement).click();
    await settle();
    await answer(callsOf('capture').at(-1), { ...CAPTURE, available: false });
    expect($('#no-project')?.textContent).toContain('open a project');
    expect(($('#save-preset') as HTMLButtonElement).disabled).toBe(true);
    unmount();
    document.body.innerHTML = '<div id="app"></div>';
    unmount = mountPanel(App, { prefix: 'plugin.form-autofill.', name: 'form autofill panel' }).unmount;
    await fromHost({ type: 'settings', strings: STRINGS, settings: SETTINGS });
    await openDialog();
    ($('#save-preset') as HTMLButtonElement).click();
    await settle();
    await fromHost({ type: 'result', id: callsOf('save').at(-1)?.id, ok: false, error: 'plugin.form-autofill.errors.presets_full', params: { name: 'Sign up' } });
    expect($('#save-refused')?.textContent).toContain('The preset was not saved: 50 presets are stored');
    expect($('[data-capture]')).not.toBeNull();
  });

  it('captures the view the command names and words a failure', async () => {
    await fromHost({ type: 'event', message: { type: 'capture', view: 3 } });
    const call = callsOf('capture').at(-1);
    expect(call?.args).toEqual({ target: 3, pending: true });
    await fromHost({ type: 'result', id: call?.id, ok: false, error: 'plugin.form-autofill.errors.capture_unsupported', params: { engine: 'firefox' } });
    expect($('#status')?.textContent).toContain('The form could not be read: A firefox view');
    expect($('#status')?.dataset.tone).toBe('error');
  });

  it('names why the store cannot be written instead of saying no project', async () => {
    // Review finding 3: a presets file that does not read is not "no project".
    ($('#take-form') as HTMLButtonElement).click();
    await settle();
    await answer(callsOf('capture').at(-1), {
      ...CAPTURE, available: false,
      storeError: { code: 'plugin.form-autofill.errors.presets_unreadable', params: { file: '.rbrowser/out/form-autofill/presets.json' } },
    });
    expect($('#no-project')).toBeNull();
    expect($('#store-failed')?.textContent).toContain('Nothing can be saved: The presets file .rbrowser/out/form-autofill/presets.json does not read');
    expect(($('#save-preset') as HTMLButtonElement).disabled).toBe(true);
    ($('#cancel-preset') as HTMLButtonElement).click();
    await settle();
    ($('#take-form') as HTMLButtonElement).click();
    await settle();
    await answer(callsOf('capture').at(-1), { ...CAPTURE, available: false, storeError: { code: 'plugins.error.not_available' } });
    expect($('#no-project')?.textContent).toContain('open a project');
    expect($('#store-failed')).toBeNull();
  });

  it('focuses the name, and Esc or Cancel closes without saving', async () => {
    await openDialog();
    expect(document.activeElement?.id).toBe('preset-name');
    ($('[data-capture]') as HTMLElement).dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    await settle();
    expect($('[data-capture]')).toBeNull();
    await openDialog();
    ($('#cancel-preset') as HTMLButtonElement).click();
    await settle();
    expect($('[data-capture]')).toBeNull();
    expect(callsOf('save')).toHaveLength(0);
  });

  it('takes the presets a save from elsewhere posts, in a group of their own', async () => {
    await fromHost({ type: 'event', message: { type: 'presets', presets: [{ name: 'From MCP', fields: 1 }] } });
    ($('#profile') as HTMLElement).click();
    await settle();
    const items = [...document.querySelectorAll<HTMLElement>('[role="option"]')].map((item) => item.textContent?.trim());
    expect(items).toEqual(['None', 'testuser', 'From MCP']);
    expect(document.body.textContent).toContain('Profiles from the settings');
    expect(document.body.textContent).toContain('Presets taken from forms');
  });

  it('chooses the preset just saved, so Fill valid uses it', async () => {
    await openDialog();
    ($('#save-preset') as HTMLButtonElement).click();
    await settle();
    await answer(callsOf('save')[0], { saved: 'Sign up', replaced: false, fields: 2, dropped: [] });
    expect($('#profile')?.textContent).toContain('Sign up');
    ($('#fill-valid') as HTMLButtonElement).click();
    await nextTick();
    expect(callsOf('fill')[0]?.args).toMatchObject({ mode: 'valid', profile: 'Sign up' });
  });
});
