import { describe, expect, it } from 'vitest';
import { nextTick } from 'vue';
import { IpcClient } from '../ipc/client';
import { createMockTransport } from '../ipc/mockTransport';
import { createShell, reportOf } from './shell';

function setup(theme: 'system' | 'light' | 'dark' = 'system', systemDark = false, textScale?: number) {
  const mock = createMockTransport({ theme, textScale });
  const root = document.createElement('html');
  const shell = createShell(new IpcClient(mock), {
    root,
    matchMedia: () => ({
      matches: systemDark,
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
    }),
  });
  return { mock, root, shell };
}

describe('the shell store', () => {
  it('loads the state and the theme on start', async () => {
    const { shell, root } = setup('dark');
    await shell.start();
    expect(shell.state.value?.views).toHaveLength(3);
    expect(shell.theme.value).toBe('dark');
    await nextTick();
    expect(root.classList.contains('dark')).toBe(true);
  });

  it('follows the system theme when set to system', async () => {
    const { shell, root } = setup('system', true);
    await shell.start();
    await nextTick();
    expect(shell.resolvedTheme.value).toBe('dark');
    expect(root.classList.contains('dark')).toBe(true);
  });

  it('runs a toolbar action and takes the pushed state', async () => {
    const { shell, mock } = setup();
    await shell.start();
    const ok = await shell.run({ command: 'view.navigate', url: 'http://localhost:5173/cart' });
    expect(ok).toBe(true);
    expect(mock.sent.at(-1)).toEqual({ type: 'navigate', url: 'http://localhost:5173/cart' });
    expect(shell.state.value?.url).toBe('http://localhost:5173/cart');
    expect(mock.reports).toEqual([]);
  });

  it('reports a refused request to the status bar (RB-080-15)', async () => {
    const { shell, mock } = setup();
    await shell.start();
    expect(await shell.run({ command: 'breakpoints.analyze' })).toBe(false);
    expect(mock.reports.at(-1)).toEqual({
      type: 'reportStatus',
      level: 'warning',
      messageKey: 'shell.error.not_implemented',
    });
    expect(await shell.run({ command: 'layout.select_preset', id: 'nope' })).toBe(false);
    expect(mock.reports.at(-1)).toMatchObject({
      type: 'reportStatus',
      level: 'error',
      messageKey: 'shell.error.request_failed',
      args: { code: '404' },
    });
  });

  it('reports a message with its setting, an error unless it says otherwise', () => {
    expect(reportOf({ key: 'sizes.conflict.notice', args: { n: 2 }, settingsKey: 'shortcuts.bindings' })).toEqual({
      type: 'reportStatus',
      level: 'error',
      messageKey: 'sizes.conflict.notice',
      args: { n: '2' },
      settingsKey: 'shortcuts.bindings',
    });
  });

  it('takes a settings push from the core', async () => {
    const { shell, mock } = setup('light');
    await shell.start();
    mock.emit({ type: 'settings', values: [{ key: 'ui.theme', value: 'dark', origin: 'user' }] });
    expect(shell.theme.value).toBe('dark');
  });

  it("draws the page at Windows' text size and follows a change (RB-090-26)", async () => {
    const { shell, root, mock } = setup('system', false, 150);
    await shell.start();
    await nextTick();
    expect(shell.textScale.value).toBe(150);
    expect(root.style.fontSize).toBe('150%');
    mock.emit({ type: 'textScale', percent: 225 });
    await nextTick();
    expect(root.style.fontSize).toBe('225%');
    mock.emit({ type: 'textScale', percent: 100 });
    await nextTick();
    expect(root.style.fontSize).toBe('');
  });

  it('asks for the address bar focus', () => {
    const { shell } = setup();
    const before = shell.focusAddressBarRequest.value;
    shell.focusAddressBar();
    expect(shell.focusAddressBarRequest.value).toBe(before + 1);
  });

  it('focuses the address bar on the core\'s uiCommand', async () => {
    const { shell, mock } = setup();
    await shell.start();
    const before = shell.focusAddressBarRequest.value;
    mock.emit({ type: 'uiCommand', id: 'view.focus_address_bar' });
    expect(shell.focusAddressBarRequest.value).toBe(before + 1);
    mock.emit({ type: 'uiCommand', id: 'app.command_palette' });
    expect(shell.focusAddressBarRequest.value).toBe(before + 1);
  });
});
