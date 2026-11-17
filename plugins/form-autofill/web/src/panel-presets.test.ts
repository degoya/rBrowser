// The profile choice in the panel (RB-090-71): always in sight, the settings' profiles and the
// presets taken from forms in groups; a stored preset chosen fills, and is renamed or deleted
// in place. The panels window is played as in panel.test.ts.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { mountPanel } from '../../../web-common/panel-ui/src/mount';
import App from './panel/App.vue';
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
  'form-autofill.activeProfile': '',
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
const refuse = (call: Posted | undefined, error: string, params: Record<string, string>): Promise<void> =>
  fromHost({ type: 'result', id: call?.id, ok: false, error, params });

async function key(target: HTMLElement, name: string): Promise<void> {
  target.dispatchEvent(new KeyboardEvent('keydown', { key: name, bubbles: true, cancelable: true }));
  await settle();
}

/** The entries the profile choice offers, opened by keyboard. */
async function offered(): Promise<HTMLElement[]> {
  await key($('#profile') as HTMLElement, 'Enter');
  return [...document.querySelectorAll<HTMLElement>('[role="option"]')];
}

/** Opens the profile choice and picks the entry that reads `label` - Enter on the entry. */
async function choose(label: string): Promise<void> {
  const option = (await offered()).find((item) => item.textContent?.trim() === label);
  if (!option) throw new Error(`no entry ${label}`);
  option.focus();
  await key(option, 'Enter');
}

beforeEach(async () => {
  posted = [];
  document.body.innerHTML = '<div id="app"></div>';
  vi.spyOn(window, 'postMessage').mockImplementation((message: unknown) => {
    posted.push(message as Posted);
  });
  unmount = mountPanel(App, { prefix: 'plugin.form-autofill.', name: 'form autofill panel' }).unmount;
  await fromHost({ type: 'settings', strings: STRINGS, settings: SETTINGS });
  await answer(callsOf('capture')[0], { pending: false });
  await answer(callsOf('presets')[0], { presets: [{ name: 'Login', fields: 2 }, { name: 'Checkout', fields: 3 }], available: true });
});

afterEach(() => {
  unmount();
  vi.restoreAllMocks();
});

describe('the profile choice', () => {
  it('is in sight without the options, and a preset chosen fills', async () => {
    expect($('#options')?.getAttribute('aria-expanded')).toBe('false');
    expect($('[data-profile-choice]')).not.toBeNull();
    expect(document.querySelector('label[for="profile"]')?.textContent).toBe('Profile');
    expect($('#rename-preset')).toBeNull();
    await choose('Checkout');
    expect($('#profile')?.textContent).toContain('Checkout');
    ($('#fill-valid') as HTMLButtonElement).click();
    await nextTick();
    expect(callsOf('fill')[0]?.args).toMatchObject({ mode: 'valid', profile: 'Checkout' });
  });

  // RB-090-72: a choice alone did nothing in sight - a button beside it fills with it.
  it('fills with the profile chosen from a button beside the choice', async () => {
    expect($('#fill-with')).toBeNull();
    expect($('[data-profile-choice]')?.textContent).toContain('Choose a profile or preset, then fill with it.');
    await choose('Checkout');
    const button = $('#fill-with') as HTMLButtonElement;
    expect(button.textContent?.trim()).toBe('Fill with Checkout');
    expect(button.hasAttribute('data-primary')).toBe(true);
    expect($('[data-profile-choice]')?.textContent).toContain('Fill valid fills with Checkout; Fill invalid uses no profile.');
    button.click();
    await nextTick();
    expect(callsOf('fill')).toHaveLength(1);
    expect(callsOf('fill')[0]?.args).toEqual({ mode: 'valid', profile: 'Checkout' });
    expect(button.disabled).toBe(true);
    await answer(callsOf('fill')[0], { mode: 'valid', seed: 7, strategy: null, profile: 'Checkout', views: [] });
    expect(button.disabled).toBe(false);
    await choose('testuser');
    expect($('#fill-with')?.textContent?.trim()).toBe('Fill with testuser');
    await choose('None');
    expect($('#fill-with')).toBeNull();
  });

  it('offers rename and delete for a preset only, not for a settings profile', async () => {
    await choose('testuser');
    expect($('#rename-preset')).toBeNull();
    expect($('#delete-preset')).toBeNull();
    await choose('Login');
    expect($('#rename-preset')?.getAttribute('aria-label')).toBe('Rename preset');
    expect($('#delete-preset')?.getAttribute('aria-label')).toBe('Delete preset');
  });

  it('renames the preset chosen and keeps it chosen', async () => {
    await choose('Login');
    ($('#rename-preset') as HTMLButtonElement).click();
    await settle();
    const input = $('#preset-new-name') as HTMLInputElement;
    expect(input.value).toBe('Login');
    expect(($('#rename-save') as HTMLButtonElement).disabled).toBe(true);
    input.value = 'Checkout';
    input.dispatchEvent(new Event('input'));
    await settle();
    expect($('#rename-taken')?.textContent).toBe('A preset named Checkout exists already.');
    expect(($('#rename-save') as HTMLButtonElement).disabled).toBe(true);
    input.value = 'testuser';
    input.dispatchEvent(new Event('input'));
    await settle();
    expect($('#rename-in-settings')?.textContent).toContain('testuser is a profile in the settings');
    input.value = ' Sign in ';
    input.dispatchEvent(new Event('input'));
    await settle();
    ($('#rename-save') as HTMLButtonElement).click();
    await settle();
    const call = callsOf('rename')[0];
    expect(call?.args).toEqual({ name: 'Login', to: 'Sign in' });
    await answer(call, { renamed: 'Login', to: 'Sign in' });
    expect($('#rename-row')).toBeNull();
    expect($('#status')?.textContent).toBe('Renamed the preset Login to Sign in.');
    expect($('#profile')?.textContent).toContain('Sign in');
    ($('#fill-valid') as HTMLButtonElement).click();
    await nextTick();
    expect(callsOf('fill')[0]?.args).toMatchObject({ profile: 'Sign in' });
  });

  it('says a refused rename in place and Esc ends the edit', async () => {
    await choose('Login');
    ($('#rename-preset') as HTMLButtonElement).click();
    await settle();
    const input = $('#preset-new-name') as HTMLInputElement;
    input.value = 'Other';
    input.dispatchEvent(new Event('input'));
    await settle();
    ($('#rename-save') as HTMLButtonElement).click();
    await settle();
    await refuse(callsOf('rename')[0], 'plugin.form-autofill.errors.preset_unknown', { name: 'Login' });
    expect($('#preset-refused')?.textContent).toBe('The preset was not renamed: No preset taken from a form is named Login.');
    ($('#rename-row') as HTMLElement).dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    await settle();
    expect($('#rename-row')).toBeNull();
    expect($('#preset-refused')).toBeNull();
  });

  it('deletes a preset only after the inline confirmation, and the choice goes back to none', async () => {
    await choose('Checkout');
    ($('#delete-preset') as HTMLButtonElement).click();
    await settle();
    expect($('#confirm-delete-preset')?.textContent).toContain('Delete the preset Checkout? Only this preset goes');
    ($('#delete-keep') as HTMLButtonElement).click();
    await settle();
    expect($('#confirm-delete-preset')).toBeNull();
    expect(callsOf('delete')).toHaveLength(0);
    ($('#delete-preset') as HTMLButtonElement).click();
    await settle();
    ($('#delete-confirm') as HTMLButtonElement).click();
    await settle();
    const call = callsOf('delete')[0];
    expect(call?.args).toEqual({ name: 'Checkout' });
    await answer(call, { deleted: 'Checkout' });
    expect($('#status')?.textContent).toBe('Deleted the preset Checkout.');
    expect($('#profile')?.textContent).toContain('None');
    expect((await offered()).map((item) => item.textContent?.trim())).toEqual(['None', 'testuser', 'Login']);
  });
});
