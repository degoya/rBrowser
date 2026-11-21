// "Copy as" and the export in the panel (RB-096-03): mounted as the panel frame mounts it, the
// panels window played by spying on postMessage (the harness of panel.test.ts).
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { mountPanel } from '../../../../web-common/panel-ui/src/mount';
import App from './App.vue';

type Op = { op?: string } & Record<string, unknown>;
type Posted = { type?: string; id?: number; tool?: string; args?: { op?: Op } };

const STRINGS = {
  'plugin.api-client.panel.code.copied': 'Copied as {language}.',
  'plugin.api-client.panel.code.notCopied': 'Select the code below and copy it.',
  'plugin.api-client.panel.code.env': 'Set first: {names}',
  'plugin.api-client.panel.code.languages.curl': 'curl',
  'plugin.api-client.panel.code.notes.oauth2': 'OAuth 2 is not in the code.',
  'plugin.api-client.panel.export.done': '{count, plural, one {# request} other {# requests}} exported to {path}.',
  'plugin.api-client.panel.export.skipped': 'Not exported: {ids}',
};

const STATE = {
  environment: 'staging',
  environments: ['staging'],
  variables: [],
  requests: [
    { id: 'users/me.http', name: 'Me', method: 'GET', url: '{{baseUrl}}/me' },
    { id: 'users/admin/all.http', name: 'All', method: 'GET', url: '{{baseUrl}}/all' },
  ],
  allowedHosts: [],
};

let posted: Posted[] = [];
let unmount: () => void = () => undefined;
const answered = new Set<number>();

const $ = (selector: string): HTMLElement | null => document.querySelector<HTMLElement>(selector);
const settle = async (): Promise<void> => {
  await new Promise((resolve) => setTimeout(resolve, 0));
  await nextTick();
};

async function fromHost(data: unknown): Promise<void> {
  window.dispatchEvent(new MessageEvent('message', { data, source: window }));
  await settle();
}

const calls = (op: string): Posted[] => posted.filter((m) => m.type === 'call' && m.tool === 'panel' && m.args?.op?.op === op);

async function reply(op: string, value: unknown, failure?: { error: string; params: Record<string, string> }): Promise<void> {
  for (const message of calls(op).filter((m) => !answered.has(m.id ?? 0))) {
    answered.add(message.id ?? 0);
    await fromHost(
      failure
        ? { type: 'result', id: message.id, ok: false, error: failure.error, params: failure.params }
        : { type: 'result', id: message.id, ok: true, value },
    );
  }
}

function clipboard(works: boolean): ReturnType<typeof vi.fn> {
  const writeText = works ? vi.fn().mockResolvedValue(undefined) : vi.fn().mockRejectedValue(new Error('denied'));
  Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
  Object.defineProperty(document, 'execCommand', { value: () => false, configurable: true });
  return writeText;
}

beforeEach(async () => {
  posted = [];
  answered.clear();
  document.body.innerHTML = '<div id="app"></div>';
  vi.spyOn(window, 'postMessage').mockImplementation((message: unknown) => {
    posted.push(message as Posted);
  });
  unmount = mountPanel(App, { prefix: 'plugin.api-client.', name: 'API client panel' }).unmount;
  await fromHost({ type: 'settings', strings: STRINGS, settings: {} });
  await reply('state', STATE);
  await reply('history', { entries: [] });
});

afterEach(() => {
  unmount();
  vi.restoreAllMocks();
});

describe('copy as and export', () => {
  it('copies the request as curl and names the variables to set', async () => {
    const writeText = clipboard(true);
    $('#copy-code')?.click();
    await settle();
    const asked = calls('code');
    expect(asked).toHaveLength(1);
    expect(asked[0]?.args?.op).toMatchObject({ op: 'code', language: 'curl', environment: 'staging' });
    expect((asked[0]?.args?.op?.request as { url?: string }).url).toBe('{{baseUrl}}/');
    await reply('code', { language: 'curl', text: "curl 'https://x/'", env: ['token'], notes: ['oauth2'], shell: 'posix', environment: 'staging' });
    await settle();
    expect(writeText).toHaveBeenCalledWith("curl 'https://x/'");
    expect($('#code-status')?.textContent).toContain('Copied as curl.');
    expect($('#code-env')?.textContent).toContain('token');
    expect($('#code-notes')?.textContent).toContain('OAuth 2 is not in the code.');
    expect($('#code pre')?.textContent).toContain("curl 'https://x/'");
  });

  it('leaves the code on screen when the clipboard is shut', async () => {
    clipboard(false);
    $('#copy-code')?.click();
    await settle();
    await reply('code', { language: 'curl', text: 'curl https://x/', env: [], notes: [], shell: 'posix', environment: null });
    await settle();
    expect($('#code-status')?.textContent).toContain('Select the code below');
    expect($('#code pre')?.textContent).toContain('curl https://x/');
    expect($('#code-env')).toBeNull();
  });

  it('exports every request as a Postman collection by default', async () => {
    $('#export-run')?.click();
    await settle();
    const asked = calls('export');
    expect(asked).toHaveLength(1);
    expect(asked[0]?.args?.op).toEqual({ op: 'export', folder: null, format: 'postman' });
    await reply('export', {
      path: '.rbrowser/out/api-client/exports/shop.postman_collection.json',
      format: 'postman',
      count: 2,
      skipped: ['users/broken.http'],
      env: [],
      notes: [],
    });
    expect($('#export-result')?.textContent).toContain('2 requests exported to .rbrowser/out/api-client/exports/shop.postman_collection.json.');
    expect($('#export-result')?.textContent).toContain('Not exported: users/broken.http');
  });
});
