// RB-096-13: every control of the API client does its work in the frame as Chromium runs it
// (`harness.ts`: the clone of postMessage, the sandbox without `allow-forms`). On 0.9.6 the
// dialog's OK, a private value's Save and "Add an environment" were form submissions the sandbox
// blocked, and Save, Save as new, Copy as code and the Variables' Save carried a reactive value
// the clone refused - "The plugin could not do that." without a line in the log.
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { $, blocked, calls, chooseTab, enter, reply, SAVED, settle, start, stop, type, valueOf } from './harness';
import { HOST_NOT_ALLOWED } from './model';

const STRINGS = {
  'plugin.api-client.panels.api-client': 'API client',
  'plugin.api-client.panel.menu.move': 'Move',
  'plugin.api-client.panel.menu.newFolder': 'New collection',
  'plugin.api-client.panel.allow.title': 'Allow {host} for this project?',
  'plugin.api-client.panel.allow.yes': 'Allow and send',
  'plugin.api-client.panel.allow.yesRun': 'Allow and run again',
  'plugin.api-client.panel.variables.otherName': 'Another name',
  'plugin.api-client.panel.side.requests': 'Requests',
  'plugin.api-client.panel.side.variables': 'Variables',
  'plugin.api-client.panel.side.import': 'Import',
  'plugin.api-client.panel.side.export': 'Export',
  'plugins.panel.call_failed': 'The plugin could not do that.',
};

beforeEach(async () => {
  await start(STRINGS);
});

afterEach(stop);

const menuItem = (text: string): HTMLElement | undefined =>
  [...document.querySelectorAll<HTMLElement>('[role="menuitem"]')].find((item) => item.textContent?.includes(text));

async function openMe(): Promise<void> {
  $('[data-id="users/me.http"]')?.click();
  await settle();
  await reply('load', { id: 'users/me.http', request: SAVED('Me', '{{baseUrl}}/me') });
}

describe('no form the sandbox blocks', () => {
  it('makes a collection with OK and with Enter, never through a form', async () => {
    $('#new-folder')?.click();
    await settle();
    expect(document.querySelector('form')).toBeNull();
    await type('#tree-dialog-text', 'orders');
    $('#tree-dialog-ok')?.click();
    await settle();
    expect(calls('createFolder')[0]?.args?.op).toEqual({ op: 'createFolder', folder: 'orders' });
    $('#new-folder')?.click();
    await settle();
    await type('#tree-dialog-text', 'carts');
    await enter('#tree-dialog-text');
    expect(calls('createFolder')[1]?.args?.op).toEqual({ op: 'createFolder', folder: 'carts' });
    expect(blocked).toBe(0);
  });

  it('moves a request with OK', async () => {
    $('[data-id="users/me.http"]')?.dispatchEvent(new KeyboardEvent('keydown', { key: 'F10', shiftKey: true, bubbles: true }));
    await settle();
    menuItem('Move')?.click();
    await settle();
    $('#tree-dialog-ok')?.click();
    await settle();
    expect(calls('move')[0]?.args?.op).toMatchObject({ op: 'move', id: 'users/me.http' });
    expect(blocked).toBe(0);
  });

  it('keeps a private value with Save', async () => {
    await type('#url', 'https://a.test/?k={{$secret password}}');
    await reply('secrets', { missing: ['password'] });
    $('[data-secret-name="password"] button')?.click();
    await settle();
    expect(document.querySelector('form')).toBeNull();
    await type('#secret-value', 'pw-2');
    $('#secret-save')?.click();
    await settle();
    expect(calls('setSecret')[0]?.args?.op).toEqual({ op: 'setSecret', name: 'password', value: 'pw-2', environment: 'staging' });
    expect(blocked).toBe(0);
  });
});

describe('the editor\'s actions reach the core', () => {
  it('saves a renamed request, saves it as new and copies it as code', async () => {
    await openMe();
    await type('#name', 'Me, renamed');
    $('#save-request')?.click();
    await settle();
    expect(calls('save')[0]?.args?.op).toMatchObject({ op: 'save', id: 'users/me.http', request: { name: 'Me, renamed', url: '{{baseUrl}}/me' } });
    await reply('save', { id: 'users/me.http' });
    $('#save-copy')?.click();
    await settle();
    expect(calls('save')[1]?.args?.op).toMatchObject({ op: 'save', id: null, folder: 'users', request: { name: 'Me, renamed' } });
    $('#copy-code')?.click();
    await settle();
    expect(calls('code')[0]?.args?.op).toMatchObject({ op: 'code', language: 'curl', environment: 'staging', request: { url: '{{baseUrl}}/me' } });
    expect(document.body.textContent).not.toContain('could not do that');
  });

  it('adds a parameter row that stays until it has a name', async () => {
    await type('#url', 'https://a.test/list');
    $('#params [data-add]')?.click();
    await settle();
    const rows = (): NodeListOf<HTMLElement> => document.querySelectorAll<HTMLElement>('#params [data-row]');
    expect(rows()).toHaveLength(1);
    const name = rows()[0]?.querySelector<HTMLInputElement>('input');
    if (name) {
      name.value = 'page';
      name.dispatchEvent(new Event('input'));
    }
    await settle();
    expect(valueOf('#url')).toBe('https://a.test/list?page');
    expect(rows()).toHaveLength(1);
  });
});

describe('the Variables tab', () => {
  it('saves the public rows', async () => {
    await chooseTab('side-tabs', 1);
    const shared = $('[data-block="$shared"]');
    const value = shared?.querySelectorAll<HTMLInputElement>('[data-variable] input:not([type="checkbox"])')[1];
    if (value) {
      value.value = 'grace';
      value.dispatchEvent(new Event('input'));
    }
    await settle();
    $('#variables-save')?.click();
    await settle();
    expect(calls('saveVariables')[0]?.args?.op).toMatchObject({ op: 'saveVariables', file: { $shared: { user: 'grace' } } });
  });

  it('makes a new environment from a name and Create, with no "Another name"', async () => {
    await chooseTab('side-tabs', 1);
    expect(document.body.textContent).not.toContain('Another name');
    await type('#variables-new-name', 'qa');
    $('#variables-new-create')?.click();
    await settle();
    expect($('[data-block="qa"]')).not.toBeNull();
    expect(valueOf('#variables-new-name')).toBe('');
    await type('#variables-new-name', 'qa');
    expect(($('#variables-new-create') as HTMLButtonElement | null)?.disabled).toBe(true);
    await type('#variables-new-name', 'perf');
    await enter('#variables-new-name');
    expect($('[data-block="perf"]')).not.toBeNull();
  });

  it('offers the project\'s environments not in the file yet, one click each', async () => {
    await chooseTab('side-tabs', 1);
    $('[data-suggested="local"]')?.click();
    await settle();
    expect($('[data-block="local"]')).not.toBeNull();
    expect($('[data-suggested="local"]')).toBeNull();
  });

  it('renames an environment apart from making one', async () => {
    await chooseTab('side-tabs', 1);
    $('[data-block="staging"] [data-rename]')?.click();
    await settle();
    await type('#variables-rename-name', 'stage');
    $('#variables-rename-ok')?.click();
    await settle();
    expect($('[data-block="stage"]')).not.toBeNull();
    $('#variables-save')?.click();
    await settle();
    expect(calls('saveVariables')[0]?.args?.op).toMatchObject({ renamed: { staging: 'stage' } });
  });
});

describe('a new host', () => {
  it('asks before a collection run sends to it, and runs again after Allow', async () => {
    $('#run-collection')?.click();
    await settle();
    await reply('runCollection', null, { error: HOST_NOT_ALLOWED, params: { host: 'localhost' } });
    expect($('#allow-question')?.textContent).toContain('Allow localhost for this project?');
    expect($('#allow')?.textContent).toContain('Allow and run again');
    expect(calls('allow')).toHaveLength(0);
    $('#allow')?.click();
    await settle();
    expect(calls('allow')[0]?.args?.op).toEqual({ op: 'allow', host: 'localhost' });
    await reply('allow', { allowed: 'localhost', allowedHosts: ['localhost'] });
    expect(calls('runCollection')).toHaveLength(2);
  });

  it('asks before a benchmark sends to it', async () => {
    await openMe();
    $('#bench-run')?.click();
    await settle();
    await reply('bench', null, { error: HOST_NOT_ALLOWED, params: { host: 'localhost' } });
    expect($('#allow-question')?.textContent).toContain('Allow localhost for this project?');
    $('#allow')?.click();
    await settle();
    await reply('allow', { allowed: 'localhost', allowedHosts: ['localhost'] });
    expect(calls('bench')).toHaveLength(2);
  });
});

describe('the side column', () => {
  it('shows its four tabs as icons named by their labels and tooltips', () => {
    const tabs = [...document.querySelectorAll<HTMLElement>('#side-tabs [role="tab"]')];
    expect(tabs).toHaveLength(4);
    expect(tabs.map((tab) => tab.textContent?.trim())).toEqual(['Requests', 'Variables', 'Import', 'Export']);
    for (const tab of tabs) {
      expect(tab.querySelector('[data-side-icon]'), tab.textContent ?? '').not.toBeNull();
      expect(tab.querySelector('[data-slot="label"]')?.className).toContain('sr-only');
    }
  });
});
