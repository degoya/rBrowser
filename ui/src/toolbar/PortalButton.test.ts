// The portal's button in the toolbar (RB-090-62): off, a click starts the portal with a QR code
// (`portal.toggle`, the core's own refusal while `portal.enabled` is off); on, the core's native
// menu below it with the QR code, the devices, a device to disconnect and the stop; the number of
// paired devices on a chip; the state in its name and tooltip with the shortcut; the keyboard; the
// state asked again on every `portalChanged`; gone with `ui.toolbar.portalButton` off; and in a
// narrow window an entry or a headed block of the overflow menu.
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import type { CommandInfo, CoreToShell, PortalDevice, PortalState } from '../ipc/protocol';
import { mountWithShell, settle } from '../test/mountShell';
import { closeNativeMenu, lastNativeMenu } from '../test/nativeMenu';
import { OVERFLOW_ORDER } from './groups';
import { overflowEntries } from './overflowEntries';
import PortalButton from './PortalButton.vue';
import { PORTAL_BUTTON_SETTING, PORTAL_ICON, type PortalSource } from './portalButton';
import ShellToolbar from './ShellToolbar.vue';

let current: VueWrapper | null = null;

afterEach(() => {
  current?.unmount();
  current = null;
});

const BUTTON = '[data-testid="portal-button"]';
const OFF: PortalState = { enabled: true, running: false, devices: [] };

function device(id: number, name: string): PortalDevice {
  return {
    id,
    name,
    ip: `192.168.1.${id}`,
    pairedAtMs: 0,
    lastSeenMs: 0,
    contacted: true,
    connected: true,
    replayed: 0,
    failed: 0,
    skipped: 0,
  };
}

function on(devices: PortalDevice[] = []): PortalState {
  return { enabled: true, running: true, address: 'http://192.168.1.10:8890', devices };
}

/** The button alone over a portal whose state the test sets, and a `portalChanged` to push. */
async function setup(state: PortalState, settings: Record<string, unknown> = {}) {
  let now = state;
  const listeners: ((message: CoreToShell) => void)[] = [];
  const source: PortalSource = {
    load: vi.fn(() => Promise.resolve(now)),
    subscribe: (listener) => {
      listeners.push(listener);
      return () => undefined;
    },
  };
  const mounted = await mountWithShell(PortalButton, { mock: { settings }, mount: { props: { source } } });
  current = mounted.wrapper;
  await settle();
  const change = async (next: PortalState): Promise<void> => {
    now = next;
    listeners.forEach((listener) => listener({ type: 'portalChanged' }));
    await settle();
  };
  return { ...mounted, source, change };
}

const runs = (sent: readonly { type: string }[]) => sent.filter((message) => message.type === 'runCommand');

describe("the portal's button in the toolbar", () => {
  it('is off, says so, and a click starts the portal - no menu', async () => {
    const { wrapper, mock } = await setup(OFF);
    const button = wrapper.get(BUTTON);
    expect(button.element.tagName).toBe('BUTTON');
    expect(button.attributes('aria-label')).toBe('Portal: off');
    expect(button.attributes('aria-haspopup')).toBeUndefined();
    expect(button.attributes('data-portal')).toBe('off');
    expect(wrapper.find('[data-icon]').attributes('data-icon')).toBe(PORTAL_ICON);
    expect(wrapper.text()).toBe('');
    await button.trigger('click');
    await settle();
    // `portal.toggle` on starts it and shows a fresh pairing QR code in the Portal panel.
    expect(runs(mock.sent)).toEqual([{ type: 'runCommand', id: 'portal.toggle', args: { enabled: true } }]);
    expect(mock.sent.some((message) => message.type === 'nativeMenu')).toBe(false);
  });

  it('is off without a source: the click still runs the command', async () => {
    const mounted = await mountWithShell(PortalButton);
    current = mounted.wrapper;
    await settle();
    expect(mounted.wrapper.get(BUTTON).attributes('aria-label')).toBe('Portal: off');
    await mounted.wrapper.get(BUTTON).trigger('click');
    expect(runs(mounted.mock.sent)).toEqual([{ type: 'runCommand', id: 'portal.toggle', args: { enabled: true } }]);
  });

  it('is on without a device: a menu with the QR code, the devices and the stop', async () => {
    const { wrapper, mock } = await setup(on());
    const button = wrapper.get(BUTTON);
    expect(button.attributes('aria-label')).toBe('Portal: on, no device paired');
    expect(button.attributes('aria-haspopup')).toBe('menu');
    expect(button.attributes('data-portal')).toBe('idle');
    expect(wrapper.text()).toBe('');
    await button.trigger('click');
    await settle();
    const menu = lastNativeMenu(mock.sent);
    expect(menu.menu).toMatch(/^portal-\d+$/);
    expect(menu.rows.map((row) => (row.kind === 'separator' ? '-' : row.label))).toEqual([
      'Show QR code to pair',
      'Devices',
      '-',
      'Disconnect a device',
      'No device is paired.',
      '-',
      'Stop portal',
    ]);
    // Nothing runs until a row is chosen; the focus comes back to the button.
    expect(runs(mock.sent)).toEqual([]);
    await closeNativeMenu(mock.sent, 'show_qr');
    expect(runs(mock.sent)).toEqual([{ type: 'runCommand', id: 'portal.show_qr' }]);
    expect(document.activeElement).toBe(button.element);
    await button.trigger('click');
    await closeNativeMenu(mock.sent, 'devices');
    await button.trigger('click');
    await closeNativeMenu(mock.sent, 'stop');
    expect(runs(mock.sent).slice(1)).toEqual([
      { type: 'runCommand', id: 'portal.devices' },
      { type: 'runCommand', id: 'portal.toggle', args: { enabled: false } },
    ]);
  });

  it('shows the number of paired devices and disconnects the one chosen', async () => {
    const { wrapper, mock } = await setup(on([device(3, 'Pixel 8'), device(4, 'iPad')]));
    const button = wrapper.get(BUTTON);
    expect(button.attributes('aria-label')).toBe('Portal: on, 2 devices paired');
    expect(button.attributes('data-portal')).toBe('paired');
    expect(wrapper.text()).toBe('2');
    await button.trigger('click');
    await settle();
    const labels = lastNativeMenu(mock.sent).rows.flatMap((row) => (row.kind === 'item' ? [row.label] : []));
    expect(labels).toEqual([
      'Show QR code to pair',
      'Devices',
      'Pixel 8 (192.168.1.3)',
      'iPad (192.168.1.4)',
      'All devices',
      'Stop portal',
    ]);
    await closeNativeMenu(mock.sent, 'device:4');
    expect(runs(mock.sent)).toEqual([{ type: 'runCommand', id: 'portal.disconnect', args: { device: 4 } }]);
    await button.trigger('click');
    await closeNativeMenu(mock.sent, 'device:all');
    expect(runs(mock.sent).at(-1)).toEqual({ type: 'runCommand', id: 'portal.disconnect' });
  });

  it('follows the portal on every portalChanged', async () => {
    const { wrapper, source, change } = await setup(OFF);
    expect(source.load).toHaveBeenCalledTimes(1);
    await change(on([device(1, 'Pixel')]));
    expect(source.load).toHaveBeenCalledTimes(2);
    expect(wrapper.get(BUTTON).attributes('aria-label')).toBe('Portal: on, 1 device paired');
    expect(wrapper.text()).toBe('1');
    await change(on());
    expect(wrapper.get(BUTTON).attributes('aria-label')).toBe('Portal: on, no device paired');
    expect(wrapper.text()).toBe('');
    await change(OFF);
    expect(wrapper.get(BUTTON).attributes('aria-label')).toBe('Portal: off');
  });

  it('opens its menu on an arrow key or F4 while on, never while off', async () => {
    const { wrapper, mock, change } = await setup(OFF);
    const element = wrapper.get(BUTTON).element as HTMLButtonElement;
    const menus = () => mock.sent.filter((message) => message.type === 'nativeMenu').length;
    const press = (key: string) => {
      const down = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true });
      element.dispatchEvent(down);
      return down;
    };
    press('ArrowDown');
    await settle();
    expect(menus()).toBe(0);
    await change(on());
    for (const [index, key] of ['ArrowDown', 'ArrowUp', 'F4'].entries()) {
      expect(press(key).defaultPrevented, key).toBe(true);
      await settle();
      expect(menus(), key).toBe(index + 1);
      await closeNativeMenu(mock.sent);
    }
    press('Tab');
    await settle();
    expect(menus()).toBe(3);
    expect(runs(mock.sent)).toEqual([]);
  });

  it("names the portal command's shortcut in its tooltip", async () => {
    const { wrapper, mock } = await setup(OFF);
    const command: CommandInfo = {
      id: 'portal.toggle',
      titleKey: 'commands.portal.toggle',
      group: 'sync',
      bindings: ['Ctrl+Shift+Space S P'],
      defaultBindings: ['Ctrl+Shift+Space S P'],
      changed: false,
      mcp: { omitted: 'protected' },
    };
    expect(wrapper.get(BUTTON).attributes('title')).toBe('Portal: off');
    mock.emit({ type: 'commands', leader: 'Ctrl+Shift+Space', commands: [command] });
    await settle();
    expect(wrapper.get(BUTTON).attributes('title')).toBe('Portal: off (Ctrl+Shift+Space S P)');
  });

  it('is gone with ui.toolbar.portalButton off', async () => {
    const { wrapper } = await setup(on(), { [PORTAL_BUTTON_SETTING]: false });
    expect(wrapper.find(BUTTON).exists()).toBe(false);
  });
});

describe("the portal's button in a narrow window", () => {

  it('stands in a group of its own that leaves the row for the overflow menu', async () => {
    expect(OVERFLOW_ORDER).toContain('portal');
    const mounted = await mountWithShell(ShellToolbar);
    current = mounted.wrapper;
    await settle();
    const group = mounted.wrapper.get(BUTTON).element.closest('[data-toolbar-group]');
    expect(group?.getAttribute('data-toolbar-group')).toBe('portal');
  });

  it('is one entry while off, which starts the portal', async () => {
    const { wrapper, mock } = await setup(OFF);
    const entries = overflowEntries([['portal', wrapper.element as HTMLElement]]);
    expect(entries.map((entry) => [entry.label, entry.heading, entry.opens])).toEqual([['Portal: off', null, false]]);
    entries[0]?.run();
    await settle();
    expect(runs(mock.sent)).toEqual([{ type: 'runCommand', id: 'portal.toggle', args: { enabled: true } }]);
  });

  it('is a headed block of its rows while on', async () => {
    const { wrapper, mock } = await setup(on([device(3, 'Pixel 8')]));
    const entries = overflowEntries([['portal', wrapper.element as HTMLElement]]);
    expect(new Set(entries.map((entry) => entry.heading))).toEqual(new Set(['Portal: on, 1 device paired']));
    expect(entries.map((entry) => entry.label)).toEqual([
      'Show QR code to pair',
      'Devices',
      'Disconnect a device / Pixel 8 (192.168.1.3)',
      'Stop portal',
    ]);
    entries[2]?.run();
    await settle();
    expect(runs(mock.sent)).toEqual([{ type: 'runCommand', id: 'portal.disconnect', args: { device: 3 } }]);
  });
});
