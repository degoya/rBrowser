// RB-080-68: the new-size picker (the palette's step "New view size ...", RB-080-46) as the owner
// met it on Windows - empty on the popup's first load, ten rows of sixteen devices and no
// "Custom size..." in sight.
import ui from '@nuxt/ui/vue-plugin';
import { mount, type VueWrapper } from '@vue/test-utils';
import { afterEach, describe, expect, it } from 'vitest';
import { nextTick } from 'vue';
import { createShellI18n } from '../i18n';
import { IpcClient } from '../ipc/client';
import type { CoreToShell, DevicePreset, ShellToCore } from '../ipc/protocol';
import type { Transport } from '../ipc/transport';
import { createShell, SHELL_KEY } from '../state/shell';
import { newViewProvider } from '../toolbar/newViewProvider';
import CommandPalette from './CommandPalette.vue';
import { PALETTE_CLIENT_KEY } from './palette';
import { RECENT_STORAGE_KEY } from './recent';
import { registerPaletteProvider } from './registry';
import { createPaletteCore } from './testCore';
import { STEP_NEW_VIEW } from './types';

/** Sixteen presets, as many as `crates/rb-config/data/devices.json` has. */
const DEVICES: DevicePreset[] = Array.from({ length: 16 }, (_, index) => ({
  id: `device-${index}`,
  kind: 'mobile',
  viewport: { width: 320 + index * 10, height: 800 },
  dpr: 2,
  touch: true,
}));

const CUSTOM = 'Custom size… (width x height, a name)';

let current: VueWrapper | null = null;
let unregister: (() => void) | null = null;

afterEach(() => {
  current?.unmount();
  current = null;
  unregister?.();
  unregister = null;
  window.localStorage.removeItem(RECENT_STORAGE_KEY);
});

async function settle(): Promise<void> {
  for (let i = 0; i < 10; i += 1) {
    await Promise.resolve();
  }
  await new Promise((resolve) => setTimeout(resolve, 0));
  await nextTick();
}

/**
 * The popup's first load as `bootstrap.ts` does it: the page mounts in the step, then the shell
 * starts - and its state comes only when `release` lets the core answer `getState`.
 */
function firstLoad() {
  unregister = registerPaletteProvider(newViewProvider);
  const core = createPaletteCore();
  const send = core.transport.send.bind(core.transport);
  core.transport.send = (message: ShellToCore): Promise<CoreToShell> =>
    message.type === 'getDevices' ? Promise.resolve({ type: 'devices', items: structuredClone(DEVICES) }) : send(message);
  let release: () => void = () => undefined;
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  const shellTransport: Transport = {
    send: async (message) => {
      if (message.type === 'getState') {
        await gate;
      }
      return core.transport.send(message);
    },
    subscribe: (listener) => core.transport.subscribe(listener),
  };
  const shell = createShell(new IpcClient(shellTransport), {
    root: document.createElement('html'),
    matchMedia: () => ({ matches: false, addEventListener: () => undefined, removeEventListener: () => undefined }),
  });
  const wrapper = mount(CommandPalette, {
    props: { subscribe: () => () => undefined, initialStep: STEP_NEW_VIEW },
    attachTo: document.body,
    global: {
      plugins: [ui, createShellI18n()],
      provide: { [SHELL_KEY]: shell, [PALETTE_CLIENT_KEY]: core.client },
    },
  });
  current = wrapper;
  const started = shell.start();
  const options = () => wrapper.findAll('[role="option"]');
  return { wrapper, options, release, started };
}

describe('the new-size picker', () => {
  it("fills on the popup's first load once the shell has the core's state", async () => {
    const { wrapper, options, release, started } = firstLoad();
    await settle();
    expect(wrapper.find('[data-testid="palette-step"]').exists()).toBe(true);
    // The palette asked the core while the shell had no state yet.
    expect(options()).toHaveLength(0);
    // Loading is not "nothing matches" on screen (RB-090-46); the screen reader hears the count.
    expect(wrapper.get('[data-slot="empty"]').text()).toBe('');
    expect(wrapper.get('[role="status"]').text()).toBe('Nothing matches');
    release();
    await started;
    await settle();
    expect(options().length).toBeGreaterThan(DEVICES.length);
    expect(options()[0]?.text()).toContain(CUSTOM);
    await wrapper.get('[role="combobox"]').setValue('zzzzqqq');
    await settle();
    expect(wrapper.get('[data-slot="empty"]').text()).toBe('Nothing matches');
  });

  it('lists every entry, the rows past the first ten included', async () => {
    const { options, release, started } = firstLoad();
    release();
    await started;
    await settle();
    const rows = options();
    const count = Number(rows[0]?.attributes('aria-setsize'));
    expect(count).toBeGreaterThan(DEVICES.length);
    expect(rows).toHaveLength(count);
    for (const device of DEVICES) {
      expect(rows.some((row) => row.text().includes(`${device.viewport.width} x 800`)), device.id).toBe(true);
    }
  });

  it('offers a typed size as a new view', async () => {
    const { wrapper, options, release, started } = firstLoad();
    release();
    await started;
    await settle();
    await wrapper.get('[role="combobox"]').setValue('1440x900');
    await nextTick();
    expect(options()[0]?.text()).toContain('New view: 1440 x 900');
    await wrapper.get('[role="combobox"]').setValue('1440x900@2');
    await nextTick();
    expect(options()[0]?.text()).toContain('New view: 1440 x 900 @2x');
  });
});
