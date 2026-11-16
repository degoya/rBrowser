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
import EmulationControls from './EmulationControls.vue';
import { EMULATION_QUERY_KEY } from './emulation';

let current: VueWrapper | null = null;

afterEach(() => {
  current?.unmount();
  current = null;
});

/** A core that knows the state, acknowledges commands and keeps one emulation per view. */
async function setup() {
  const sent: ShellToCore[] = [];
  const emulations = new Map<number, Emulation>();
  const base: Emulation = { width: 393, height: 852, dpr: 3, mobile: true, touch: true, media: {} };
  const transport: Transport = {
    async send(message) {
      sent.push(message);
      if (message.type === 'getState') {
        return { type: 'state', ...sampleState() } as CoreToShell;
      }
      if (message.type === 'runCommand') {
        const args = (message.args ?? {}) as { fields?: Partial<Emulation>; target?: string };
        const views = args.target === 'all' ? [1, 2, 3] : [1];
        for (const view of views) {
          const before = emulations.get(view) ?? base;
          emulations.set(
            view,
            message.id === 'emulation.reset_view' ? base : { ...before, ...args.fields },
          );
        }
        return { type: 'ack' };
      }
      return { type: 'settings', values: [] };
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
  const query = (view: number) => Promise.resolve(emulations.get(view) ?? base);
  const wrapper = mount(EmulationControls, {
    attachTo: document.body,
    global: {
      plugins: [ui, createShellI18n()],
      provide: { [SHELL_KEY]: shell, [EMULATION_QUERY_KEY]: query },
    },
  });
  current = wrapper;
  await settle();
  return { wrapper, sent, emulations };
}

function commands(sent: ShellToCore[]) {
  return sent.filter((message) => message.type === 'runCommand');
}

describe('the emulation controls', () => {
  it('are a named group of named buttons', async () => {
    const { wrapper } = await setup();
    const group = wrapper.get('[role="group"]');
    expect(group.attributes('aria-label')).toBe('Emulation of the active view');
    for (const button of group.findAll('button')) {
      expect(button.attributes('aria-label')).toBeTruthy();
      expect(button.attributes('title')).toContain('Shift-click');
    }
  });

  // RB-080-95: the sun and moon are the pages' colour scheme, told apart from rBrowser's theme.
  it("name the pages' colour scheme, not rBrowser's own theme", async () => {
    const { wrapper } = await setup();
    for (const id of ['emulation-light', 'emulation-dark']) {
      const title = wrapper.get(`[data-testid="${id}"]`).attributes('title') ?? '';
      expect(title).toMatch(/^Page colour scheme: (light|dark)/);
      expect(title).toContain("not rBrowser's own theme");
    }
  });

  it('set the colour scheme of the active view with a click', async () => {
    const { wrapper, sent, emulations } = await setup();
    await wrapper.get('[data-testid="emulation-dark"]').trigger('click');
    await settle();
    expect(commands(sent)).toEqual([
      { type: 'runCommand', id: 'emulation.set', args: { fields: { colorScheme: 'dark' } } },
    ]);
    expect(emulations.get(1)?.colorScheme).toBe('dark');
    expect(emulations.has(2)).toBe(false);
    expect(wrapper.get('[data-testid="emulation-dark"]').attributes('aria-pressed')).toBe('true');
  });

  it('apply it to every view with Shift-click', async () => {
    const { wrapper, sent, emulations } = await setup();
    await wrapper.get('[data-testid="emulation-dark"]').trigger('click', { shiftKey: true });
    await settle();
    expect(commands(sent)[0]).toEqual({
      type: 'runCommand',
      id: 'emulation.set',
      args: { fields: { colorScheme: 'dark' }, target: 'all' },
    });
    expect([1, 2, 3].map((view) => emulations.get(view)?.colorScheme)).toEqual([
      'dark',
      'dark',
      'dark',
    ]);
  });

  it('toggle reduced motion and reset to the breakpoint', async () => {
    const { wrapper, sent } = await setup();
    const motion = wrapper.get('[data-testid="emulation-reduced-motion"]');
    await motion.trigger('click');
    await settle();
    expect(motion.attributes('aria-pressed')).toBe('true');
    await motion.trigger('click');
    await settle();
    expect(motion.attributes('aria-pressed')).toBe('false');
    await wrapper.get('[data-testid="emulation-reset"]').trigger('click', { shiftKey: true });
    await settle();
    expect(commands(sent).map((message) => message.type === 'runCommand' && message.args)).toEqual([
      { fields: { reducedMotion: true } },
      { fields: { reducedMotion: false } },
      { target: 'all' },
    ]);
  });
});
