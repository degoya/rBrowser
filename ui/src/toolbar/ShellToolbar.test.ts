import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { mountWithShell, settle } from '../test/mountShell';
import { closeNativeMenu } from '../test/nativeMenu';
import ShellToolbar from './ShellToolbar.vue';

let current: VueWrapper | null = null;

async function setup() {
  const mounted = await mountWithShell(ShellToolbar);
  current = mounted.wrapper;
  await settle();
  return mounted;
}

afterEach(() => {
  current?.unmount();
  current = null;
});

function accessibleName(el: Element): string {
  const labelledBy = el.getAttribute('aria-labelledby');
  const byLabel = el.id ? document.querySelector(`label[for="${el.id}"]`)?.textContent : null;
  return (
    el.getAttribute('aria-label') ??
    (labelledBy ? document.getElementById(labelledBy)?.textContent : null) ??
    byLabel ??
    el.textContent ??
    ''
  ).trim();
}

describe('the toolbar', () => {
  it('is a labelled toolbar whose every control is reachable with Tab and named', async () => {
    const { wrapper } = await setup();
    const toolbar = wrapper.get('[role="toolbar"]');
    expect(toolbar.attributes('aria-label')).toBe('rBrowser toolbar');
    const controls = toolbar.element.querySelectorAll('button, input, select, [role="switch"]');
    // back, forward, reload, address, sync, layout, environment, analyse, settings
    expect(controls.length).toBeGreaterThanOrEqual(9);
    for (const control of controls) {
      expect(control.getAttribute('tabindex')).not.toBe('-1');
      expect(accessibleName(control), control.outerHTML).not.toBe('');
    }
  });

  it.each([
    ['back', { type: 'back' }],
    ['forward', { type: 'forward' }],
    ['reload', { type: 'reload', ignoreCache: false }],
    ['settings', { type: 'openSettings' }],
  ])('the %s button sends its command', async (testId, message) => {
    const { wrapper, mock } = await setup();
    await wrapper.get(`[data-testid="${testId}"]`).trigger('click');
    await settle();
    expect(mock.sent.at(-1)).toEqual(message);
  });

  it('opens a typed address in every view on Enter', async () => {
    const { wrapper, mock } = await setup();
    const input = wrapper.get('input[type="text"]');
    expect((input.element as HTMLInputElement).value).toBe('http://localhost:5173/');
    await input.setValue('localhost:5173/cart');
    await wrapper.get('form').trigger('submit');
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'navigate', url: 'http://localhost:5173/cart' });
    expect(mock.state.views.every((v) => v.url === 'http://localhost:5173/cart')).toBe(true);
  });

  it('refuses a javascript: address and reports it to the status bar', async () => {
    const { wrapper, mock } = await setup();
    const navigations = mock.sent.filter((m) => m.type === 'navigate').length;
    await wrapper.get('input[type="text"]').setValue('javascript:alert(1)');
    await wrapper.get('form').trigger('submit');
    await settle();
    expect(mock.sent.filter((m) => m.type === 'navigate')).toHaveLength(navigations);
    expect(mock.reports.at(-1)).toMatchObject({ type: 'reportStatus', level: 'warning' });
    expect(wrapper.get('input[type="text"]').attributes('aria-invalid')).toBe('true');
  });

  it('restores the current address on Escape', async () => {
    const { wrapper } = await setup();
    const input = wrapper.get('input[type="text"]');
    await input.setValue('somewhere.example');
    await input.trigger('keydown', { key: 'Escape' });
    expect((input.element as HTMLInputElement).value).toBe('http://localhost:5173/');
  });

  it('switches sync off for all views', async () => {
    const { wrapper, mock } = await setup();
    const toggle = wrapper.get('[role="switch"]');
    expect(toggle.attributes('aria-checked')).toBe('true');
    await toggle.trigger('click');
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'setSync', feature: 'all', enabled: false });
    expect(wrapper.get('[role="switch"]').attributes('aria-checked')).toBe('false');
  });

  it("selects a layout preset and an environment from the core's native menus", async () => {
    const { wrapper, mock } = await setup();
    expect(wrapper.find('select').exists()).toBe(false);
    await wrapper.get('[data-testid="layout"] button').trigger('click');
    await settle();
    await closeNativeMenu(mock.sent, 'focus');
    expect(mock.sent.at(-1)).toEqual({ type: 'selectLayoutPreset', id: 'focus' });
    await wrapper.get('[data-testid="environment"] button').trigger('click');
    await settle();
    await closeNativeMenu(mock.sent, 'staging');
    expect(mock.sent.at(-1)).toEqual({ type: 'selectEnvironment', id: 'staging' });
    expect((wrapper.get('input[type="text"]').element as HTMLInputElement).value).toBe(
      'https://staging.example.com/',
    );
  });

  it('reports a refused command to the status bar', async () => {
    const { wrapper, mock } = await setup();
    await wrapper.get('[data-testid="analyze"]').trigger('click');
    await settle();
    expect(mock.reports.at(-1)).toEqual({
      type: 'reportStatus',
      level: 'warning',
      messageKey: 'shell.error.not_implemented',
    });
  });

  it('shows no message text: messages go to the status bar (RB-080-15)', async () => {
    const { wrapper, mock } = await setup();
    mock.emit({
      type: 'notification',
      level: 'warning',
      messageKey: 'diagnostics.cefAge.notice',
      args: { version: '156.0.26', date: '2026-09-01', days: '70' },
      settingsKey: 'updates.cefAgeWarning',
    });
    await settle();
    expect(wrapper.find('[data-testid="status"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="status-settings"]').exists()).toBe(false);
    expect(wrapper.text()).not.toContain('CEF 156.0.26');
  });

  it('focuses the address bar on the focus command', async () => {
    const { wrapper, shell } = await setup();
    shell.focusAddressBar();
    await settle();
    expect(document.activeElement).toBe(wrapper.get('input[type="text"]').element);
  });

  // RB-080-52, the cause: the row is one flex line that hides what runs over (`overflow-hidden`),
  // every group in it refuses to shrink (`shrink-0`) and the address bar was the one item that
  // did (`min-w-0 flex-1`) - so a narrow window took its whole deficit from the address bar, down
  // to 0 px, and cut the rest off at the right edge.
  it('gives the address bar a floor in a row whose other items do not shrink', async () => {
    const { wrapper } = await setup();
    const toolbar = wrapper.get('[role="toolbar"]');
    expect(toolbar.classes()).toEqual(expect.arrayContaining(['flex', 'overflow-hidden']));
    const form = wrapper.get('form');
    expect(form.classes()).toContain('flex-1');
    for (const item of toolbar.element.children) {
      if (item !== form.element) {
        expect(item.classList.contains('shrink-0'), item.outerHTML.slice(0, 80)).toBe(true);
      }
    }
    // 240 px, never 0.
    expect(form.classes()).not.toContain('min-w-0');
    expect(form.classes()).toContain('min-w-60');
  });

  it('switches the logo with the theme', async () => {
    const { wrapper, shell } = await setup();
    const light = wrapper.get('img').attributes('src');
    shell.theme.value = 'dark';
    await settle();
    expect(wrapper.get('img').attributes('src')).not.toBe(light);
    expect(wrapper.get('img').attributes('alt')).toBe('rBrowser');
  });
});
