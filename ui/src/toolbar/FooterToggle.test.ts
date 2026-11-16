// The breakpoint bar's switch in the toolbar (RB-080-60; formerly the footer, RB-090-11):
// `ui.view_footer.toggle` as a button that shows whether the bar is on, with the shortcut in its
// tooltip, gone with `ui.toolbar.footerToggle` off, and an entry of the overflow menu in a narrow
// window.
import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import type { CommandInfo } from '../ipc/protocol';
import { createShellI18n } from '../i18n';
import { mountWithShell, settle } from '../test/mountShell';
import { FOOTER_COMMAND, FOOTER_TOGGLE_SETTING, FOOTER_VISIBLE_SETTING } from './footerToggle';
import { OVERFLOW_ORDER } from './groups';
import { overflowEntries } from './overflowEntries';
import ShellToolbar from './ShellToolbar.vue';

let current: VueWrapper | null = null;

async function setup(settings: Record<string, unknown> = {}) {
  const mounted = await mountWithShell(ShellToolbar, { mock: { settings } });
  current = mounted.wrapper;
  await settle();
  return mounted;
}

afterEach(() => {
  current?.unmount();
  current = null;
});

const SWITCH = '[data-testid="footer-toggle"]';
/**
 * The switch's name as the catalogue says it: "Breakpoint bar under every view" since RB-090-11
 * (`scripts/tests/breakpoint-bar-name.sh` holds the wording, with a wave's changed texts applied).
 */
const LABEL = createShellI18n().global.t('shell.toolbar.footer');

describe("the footer's switch in the toolbar", () => {
  it('runs ui.view_footer.toggle and is on while the footer shows', async () => {
    const { wrapper, mock } = await setup({ [FOOTER_VISIBLE_SETTING]: true });
    const button = wrapper.get(SWITCH);
    expect(button.attributes('role')).toBe('switch');
    expect(button.attributes('aria-checked')).toBe('true');
    expect(button.attributes('aria-label')).toBe(LABEL);
    expect(button.element.closest('[data-toolbar-group="footer"]')).not.toBeNull();
    await button.trigger('click');
    await settle();
    expect(mock.sent).toContainEqual({ type: 'runCommand', id: FOOTER_COMMAND });
  });

  it('is off while the footer is hidden and follows a changed setting', async () => {
    const settings: Record<string, unknown> = { [FOOTER_VISIBLE_SETTING]: false };
    const { wrapper, mock } = await setup(settings);
    expect(wrapper.get(SWITCH).attributes('aria-checked')).toBe('false');
    settings[FOOTER_VISIBLE_SETTING] = true;
    mock.emit({ type: 'settingsChanged', keys: [FOOTER_VISIBLE_SETTING] });
    await settle();
    expect(wrapper.get(SWITCH).attributes('aria-checked')).toBe('true');
  });

  it('counts the footer as shown when the core names no value', async () => {
    const { wrapper } = await setup();
    expect(wrapper.get(SWITCH).attributes('aria-checked')).toBe('true');
  });

  it('is gone with ui.toolbar.footerToggle off', async () => {
    const { wrapper } = await setup({ [FOOTER_TOGGLE_SETTING]: false });
    expect(wrapper.find(SWITCH).exists()).toBe(false);
    expect(wrapper.get('[data-toolbar-group="footer"]').element.children).toHaveLength(0);
  });

  it("names the command's shortcut in its tooltip before any hover, and follows an edit", async () => {
    const { wrapper, mock } = await setup();
    const command = (bindings: string[]): CommandInfo => ({
      id: FOOTER_COMMAND,
      titleKey: `commands.${FOOTER_COMMAND}`,
      group: 'overlays',
      bindings,
      defaultBindings: ['Ctrl+Shift+Space O B'],
      changed: false,
      mcp: { omitted: 'display switch' },
    });
    // Asked for once when it mounts; the mock core answers no `listCommands`: the name alone.
    expect(mock.sent.filter((m) => m.type === 'listCommands')).toHaveLength(1);
    const button = wrapper.get(SWITCH);
    expect(button.attributes('title')).toBe(LABEL);
    // The core pushes the registry's commands (a binding edited, a plugin's commands added).
    mock.emit({ type: 'commands', leader: 'Ctrl+Shift+Space', commands: [command(['Ctrl+Shift+Space O B'])] });
    await settle();
    expect(button.attributes('title')).toBe(`${LABEL} (Ctrl+Shift+Space O B)`);
    mock.emit({ type: 'commands', leader: 'Ctrl+Shift+Space', commands: [command(['Ctrl+Shift+Space O F'])] });
    await settle();
    expect(button.attributes('title')).toBe(`${LABEL} (Ctrl+Shift+Space O F)`);
    await button.trigger('pointerenter');
    await button.trigger('focus');
    await settle();
    expect(mock.sent.filter((m) => m.type === 'listCommands')).toHaveLength(1);
  });

  it('leaves the row for the overflow menu before the layout presets, as an entry with its state', async () => {
    expect(OVERFLOW_ORDER).toContain('footer');
    expect(OVERFLOW_ORDER.indexOf('footer')).toBeLessThan(OVERFLOW_ORDER.indexOf('layout'));
    const { wrapper, mock } = await setup({ [FOOTER_VISIBLE_SETTING]: true });
    const group = wrapper.get('[data-toolbar-group="footer"]').element as HTMLElement;
    const texts = { on: (label: string) => `${label}: on`, off: (label: string) => `${label}: off` };
    const entries = overflowEntries([['footer', group]], texts);
    expect(entries.map((entry) => entry.label)).toEqual([`${LABEL}: on`]);
    const off = await setup({ [FOOTER_VISIBLE_SETTING]: false });
    const hidden = off.wrapper.get('[data-toolbar-group="footer"]').element as HTMLElement;
    expect(overflowEntries([['footer', hidden]], texts).map((entry) => entry.label)).toEqual([`${LABEL}: off`]);
    off.wrapper.unmount();
    entries[0]?.run();
    await settle();
    expect(mock.sent).toContainEqual({ type: 'runCommand', id: FOOTER_COMMAND });
  });
});
