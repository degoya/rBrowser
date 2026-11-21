// The API client's layout and collections (RB-096-11) in its frame (`harness.ts`): the header
// above the request's tabs, the side column's tabs, a collection made and a request in it, the
// context menu by key, the Variables tab, a `{{$secret}}` without a value set from the panel, a
// settings link.
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { $, calls, chooseTab, posted, reply, SAVED, settle, start, stop, type, valueOf } from './harness';

const STRINGS = {
  'plugin.api-client.panels.api-client': 'API client',
  'plugin.api-client.panel.menu.newFolder': 'New collection',
  'plugin.api-client.panel.menu.duplicate': 'Duplicate',
  'plugin.api-client.panel.menu.delete': 'Delete',
  'plugin.api-client.panel.dialog.deleteFolder': 'Delete the collection {folder} with {count, plural, one {# request} other {# requests}} in it?',
  'plugin.api-client.panel.secretValue.title': 'No value yet: {names}',
  'plugin.api-client.panel.openSetting': 'Open the setting',
  'plugin.api-client.panel.bench.hint': 'Sends it a few times (Settings > Network > API client).',
  'plugin.api-client.panel.allow.title': 'Send requests to {host}?',
  'plugin.api-client.errors.secrets_unreadable': 'The private values (Settings > Network > API client) do not read: {reason}',
  'plugin.api-client.errors.too_many_hosts': 'The project allows the most hosts it can keep; remove one in Settings > Network > API client.',
  'plugin.api-client.errors.host_not_allowed': 'Not allowed: {host}.',
};

beforeEach(async () => {
  await start(STRINGS);
});

afterEach(stop);

describe('the header above the tabs', () => {
  it('holds the URL, the save row, Copy as code and Benchmark before the tabs, whichever tab is shown', async () => {
    const header = $('#request-header');
    const tabs = $('#request-tabs');
    expect(header && tabs && header.compareDocumentPosition(tabs) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    for (const id of ['#url', '#send', '#name', '#save-request', '#save-copy', '#delete', '#copy-code', '#code-language', '#bench-run']) {
      expect(header?.querySelector(id), id).not.toBeNull();
    }
    const before = header?.innerHTML;
    await chooseTab('request-tabs', 2);
    expect(header?.innerHTML).toBe(before);
  });

  it('keeps the environment picker even without environments', async () => {
    stop();
    await start(STRINGS, { environment: null, environments: [], variables: [], requests: [], folders: [], allowedHosts: [] });
    expect($('#environment')).not.toBeNull();
  });
});

describe('the collections', () => {
  it('makes a collection, then a new request saved in it', async () => {
    $('#new-folder')?.click();
    await settle();
    await type('#tree-dialog-text', 'orders');
    ($('#tree-dialog') as HTMLFormElement).requestSubmit();
    await settle();
    expect(calls('createFolder')[0]?.args?.op).toEqual({ op: 'createFolder', folder: 'orders' });
    await reply('createFolder', { folder: 'orders' });
    expect(calls('state')).toHaveLength(2);
    await reply('state', { requests: [], folders: ['orders'], environments: [], variables: [], allowedHosts: [], environment: null });
    expect($('[data-folder-row="orders"]')).not.toBeNull();
  });

  it('opens a request row\'s menu by key and duplicates it', async () => {
    const row = $('[data-id="users/me.http"]');
    row?.dispatchEvent(new KeyboardEvent('keydown', { key: 'F10', shiftKey: true, bubbles: true }));
    await settle();
    const duplicate = [...document.querySelectorAll<HTMLElement>('[role="menuitem"]')].find((item) => item.textContent?.includes('Duplicate'));
    expect(duplicate).toBeDefined();
    duplicate?.click();
    await settle();
    expect(calls('duplicate')[0]?.args?.op).toMatchObject({ op: 'duplicate', id: 'users/me.http' });
  });

  it('asks before a collection goes and names how many requests go with it', async () => {
    $('[data-folder-row="users"]')?.click();
    await settle();
    const remove = [...document.querySelectorAll<HTMLElement>('[role="menuitem"]')].find((item) => item.textContent?.includes('Delete'));
    remove?.click();
    await settle();
    expect($('#tree-dialog')?.textContent).toContain('Delete the collection users with 1 request in it?');
    expect(calls('deleteFolder')).toHaveLength(0);
    ($('#tree-dialog') as HTMLFormElement).requestSubmit();
    await settle();
    expect(calls('deleteFolder')[0]?.args?.op).toEqual({ op: 'deleteFolder', folder: 'users' });
  });
});

describe('variables and secrets', () => {
  it('saves a new private row into the private values, never into the file', async () => {
    await chooseTab('side-tabs', 1);
    const staging = $('[data-block="staging"]');
    staging?.querySelector<HTMLElement>('[data-add]')?.click();
    await settle();
    const rows = staging?.querySelectorAll<HTMLElement>('[data-variable]') ?? [];
    const last = rows[rows.length - 1];
    const [name, value] = [...(last?.querySelectorAll<HTMLInputElement>('input:not([type="checkbox"])') ?? [])];
    for (const [field, text] of [[name, 'password'], [value, 'pw-1']] as const) {
      if (field) {
        field.value = text;
        field.dispatchEvent(new Event('input'));
      }
    }
    await settle();
    (last?.querySelector('[role="checkbox"]') as HTMLElement | null)?.click();
    await settle();
    $('#variables-save')?.click();
    await settle();
    const op = calls('saveVariables')[0]?.args?.op;
    expect(op?.file).toEqual({ $shared: { user: 'ada' }, staging: { path: '/v1' } });
    expect(op?.private).toEqual({ staging: { password: 'pw-1' } });
  });

  it('says a $secret without a value before the send and sets it', async () => {
    await type('#url', 'https://a.test/?k={{$secret password}}');
    expect(calls('secrets').at(-1)?.args?.op).toMatchObject({ op: 'secrets', names: ['password'], environment: 'staging' });
    await reply('secrets', { missing: ['password'] });
    expect($('#secrets-missing')?.textContent).toContain('No value yet: password');
    $('[data-secret-name="password"] button')?.click();
    await settle();
    await type('#secret-value', 'pw-2');
    ($('#secret-value')?.closest('form') as HTMLFormElement).requestSubmit();
    await settle();
    expect(calls('setSecret')[0]?.args?.op).toEqual({ op: 'setSecret', name: 'password', value: 'pw-2', environment: 'staging' });
  });

  it('marks a variable without a value in the environment', async () => {
    await type('#url', '{{baseUrl}}/{{path}}/{{nobody}}');
    expect($('[data-hint="baseUrl"]')?.getAttribute('data-state')).toBe('neutral');
    expect($('[data-hint="path"]')?.getAttribute('data-state')).toBe('neutral');
    expect($('[data-hint="nobody"]')?.getAttribute('data-state')).toBe('error');
  });
});

describe('settings links (RB-094-21)', () => {
  const lastPost = () => posted.at(-1);

  it('opens api-client.benchRepeats from the benchmark hint', async () => {
    $('[data-id="users/me.http"]')?.click();
    await settle();
    await reply('load', { id: 'users/me.http', request: SAVED('Me', '{{baseUrl}}/me') });
    expect(valueOf('#url')).toBe('{{baseUrl}}/me');
    expect($('#bench-hint')?.textContent).toContain('Settings > Network > API client');
    $('#bench-hint [data-setting]')?.click();
    expect(lastPost()).toEqual({ target: 'host', action: 'open_settings', key: 'api-client.benchRepeats' });
  });

  it('opens api-client.allowedHosts from the question before a new host', async () => {
    await type('#url', 'https://new.test/');
    $('#send')?.click();
    await settle();
    await reply('send', null, { error: 'plugin.api-client.errors.host_not_allowed', params: { host: 'new.test' } });
    $('#allow-question [data-setting]')?.click();
    expect(lastPost()).toEqual({ target: 'host', action: 'open_settings', key: 'api-client.allowedHosts' });
  });

  it('links a failure that names the settings: secrets_unreadable and too_many_hosts', async () => {
    for (const [code, key] of [
      ['secrets_unreadable', 'api-client.secrets'],
      ['too_many_hosts', 'api-client.allowedHosts'],
    ] as const) {
      $('#refresh')?.click();
      await settle();
      await reply('state', null, { error: `plugin.api-client.errors.${code}`, params: { reason: 'x' } });
      await reply('variables', { file: null, private: {}, baseUrls: {} });
      expect($(`[data-testid="setting-link-${key}"]`), code).not.toBeNull();
      $(`[data-testid="setting-link-${key}"]`)?.click();
      expect(lastPost()).toEqual({ target: 'host', action: 'open_settings', key });
    }
  });
});

describe('keyboard and names (RB-090-08)', () => {
  /** The controls of the panel without an accessible name. */
  function unnamed(): string[] {
    const controls = document.querySelectorAll<HTMLElement>('button, input:not([type="hidden"]):not([tabindex="-1"]), textarea, [role="combobox"], [role="checkbox"], [role="tab"]');
    return [...controls]
      .filter((control) => !control.closest('[aria-hidden="true"]'))
      .filter((control) => {
        const labelled = control.getAttribute('aria-labelledby');
        const byLabel = control.id ? document.querySelector(`label[for="${control.id}"]`) : null;
        return !(control.getAttribute('aria-label')?.trim() || control.textContent?.trim() || control.getAttribute('title') || labelled || byLabel || control.closest('label'));
      })
      .map((control) => control.outerHTML.slice(0, 120));
  }

  it('names every control of every side tab and reaches them with Tab', async () => {
    for (const index of [0, 1, 2, 3]) {
      await chooseTab('side-tabs', index);
      expect(unnamed(), `side tab ${index}`).toEqual([]);
    }
    const buttons = [...document.querySelectorAll<HTMLElement>('#request-header button')];
    expect(buttons.every((button) => button.tabIndex >= 0)).toBe(true);
  });
});
