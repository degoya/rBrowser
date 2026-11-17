import { afterEach, describe, expect, it } from 'vitest';
import ui from '@nuxt/ui/vue-plugin';
import { mount, type VueWrapper } from '@vue/test-utils';
import { IpcClient } from '../ipc/client';
import { sampleState } from '../ipc/mockState';
import type { CoreToShell, Emulation, ShellToCore } from '../ipc/protocol';
import type { Transport } from '../ipc/transport';
import { createShellI18n } from '../i18n';
import { createShell, SHELL_KEY } from '../state/shell';
import { settle } from '../test/mountShell';
import EmulationExtras from './EmulationExtras.vue';

let current: VueWrapper | null = null;

afterEach(() => {
  current?.unmount();
  current = null;
});

const base: Emulation = { width: 393, height: 852, dpr: 3, mobile: true, touch: true, media: {} };

async function setup(emulation: Emulation = base) {
  const sent: ShellToCore[] = [];
  const transport: Transport = {
    async send(message) {
      sent.push(message);
      if (message.type === 'getState') {
        return { type: 'state', ...sampleState() } as CoreToShell;
      }
      return message.type === 'runCommand' ? { type: 'ack' } : { type: 'settings', values: [] };
    },
    subscribe: () => () => undefined,
  };
  const shell = createShell(new IpcClient(transport), {
    root: document.createElement('html'),
    matchMedia: () => ({
      matches: false,
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
    }),
  });
  await shell.start();
  const wrapper = mount(EmulationExtras, {
    attachTo: document.body,
    props: { current: emulation, disabled: false },
    global: { plugins: [ui, createShellI18n()], provide: { [SHELL_KEY]: shell } },
  });
  current = wrapper;
  await settle();
  await wrapper.get('[data-testid="emulation-extras"]').trigger('click');
  await settle();
  return { wrapper, sent };
}

/** The text field of a `UInput` by its test id (the attribute may sit on the input itself). */
function field(wrapper: VueWrapper, id: string) {
  const found = wrapper.get(`[data-testid="${id}"]`);
  return found.element.tagName === 'INPUT' ? found : found.get('input');
}

function commands(sent: ShellToCore[]) {
  return sent.filter((message) => message.type === 'runCommand');
}

describe('the emulation extras', () => {
  it('open a named panel from a named button', async () => {
    const { wrapper } = await setup();
    const trigger = wrapper.get('[data-testid="emulation-extras"]');
    expect(trigger.attributes('aria-label')).toBeTruthy();
    expect(trigger.attributes('title')).toContain('Shift-click');
    expect(wrapper.find('[data-testid="emulation-extras-panel"]').exists()).toBe(true);
  });

  it('set a preset position, a typed one and none', async () => {
    const { wrapper, sent } = await setup();
    await wrapper.get('[data-testid="geo-tokyo"]').trigger('click', { shiftKey: true });
    await field(wrapper, 'geo-latitude').setValue('52.52');
    await field(wrapper, 'geo-longitude').setValue('13,4');
    await wrapper.get('[data-testid="geo-apply"]').trigger('click');
    await wrapper.get('[data-testid="geo-off"]').trigger('click');
    await settle();
    expect(commands(sent)).toEqual([
      {
        type: 'runCommand',
        id: 'view.emulate.geolocation',
        args: { preset: 'tokyo', target: 'all' },
      },
      {
        type: 'runCommand',
        id: 'view.emulate.geolocation',
        args: { geolocation: { latitude: 52.52, longitude: 13.4 } },
      },
      { type: 'runCommand', id: 'view.emulate.geolocation', args: { geolocation: null } },
    ]);
  });

  it('set the time zone, the CPU throttling and offline, and add a print view', async () => {
    const { wrapper, sent } = await setup();
    await field(wrapper, 'tz-input').setValue(' Asia/Tokyo ');
    await wrapper.get('[data-testid="tz-apply"]').trigger('click');
    await wrapper.get('[data-testid="cpu-4"]').trigger('click');
    await wrapper.get('[data-testid="offline"]').trigger('click', { shiftKey: true });
    await wrapper.get('[data-testid="print-A4"]').trigger('click');
    await settle();
    expect(commands(sent).map((message) => message.type === 'runCommand' && [message.id, message.args])).toEqual([
      ['view.emulate.timezone', { timezone: 'Asia/Tokyo' }],
      ['view.emulate.cpu', { rate: 4 }],
      ['view.emulate.offline', { offline: true, target: 'all' }],
      ['view.add_print', { paper: 'A4' }],
    ]);
  });

  it('show the active view state', async () => {
    const { wrapper } = await setup({
      ...base,
      geolocation: { latitude: 52.52, longitude: 13.405, accuracy: 50 },
      cpuThrottling: 4,
      network: { offline: true, latencyMs: 0, downloadKbps: 0, uploadKbps: 0 },
    });
    expect(wrapper.get('[data-testid="geo-berlin"]').attributes('aria-pressed')).toBe('true');
    expect(wrapper.get('[data-testid="cpu-4"]').attributes('aria-pressed')).toBe('true');
    expect(wrapper.get('[data-testid="offline"]').attributes('aria-pressed')).toBe('true');
  });
});
