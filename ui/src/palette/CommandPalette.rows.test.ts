// How many rows the palette mounts (RB-080-68 review): every match of a short list, at most fifty
// of a long one with a note on the rest; the selection is always a row.
import ui from '@nuxt/ui/vue-plugin';
import { mount, type VueWrapper } from '@vue/test-utils';
import { afterEach, describe, expect, it } from 'vitest';
import { nextTick } from 'vue';
import { createShellI18n } from '../i18n';
import type { CoreToShell } from '../ipc/protocol';
import { createShell, SHELL_KEY } from '../state/shell';
import CommandPalette from './CommandPalette.vue';
import { PALETTE_COMMAND } from './constants';
import { PALETTE_CLIENT_KEY } from './palette';
import { RECENT_STORAGE_KEY } from './recent';
import { registerPaletteProvider } from './registry';
import { createPaletteCore } from './testCore';
import type { PaletteEntry } from './types';

const MANY = 120;

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

async function setup() {
  unregister = registerPaletteProvider({
    id: 'many',
    entries: (): PaletteEntry[] =>
      Array.from({ length: MANY }, (_, index) => ({
        key: `many:${index}`,
        scope: 'command',
        title: `Row ${index}`,
        action: { kind: 'send', message: { type: 'runCommand', id: `many.${index}` } },
      })),
  });
  const core = createPaletteCore();
  const shell = createShell(core.client, {
    root: document.createElement('html'),
    matchMedia: () => ({ matches: false, addEventListener: () => undefined, removeEventListener: () => undefined }),
  });
  await shell.start();
  let push: (message: CoreToShell) => void = () => undefined;
  const wrapper = mount(CommandPalette, {
    props: {
      subscribe: (listener: (message: CoreToShell) => void) => {
        push = listener;
        return () => undefined;
      },
    },
    attachTo: document.body,
    global: {
      plugins: [ui, createShellI18n()],
      provide: { [SHELL_KEY]: shell, [PALETTE_CLIENT_KEY]: core.client },
    },
  });
  current = wrapper;
  push({ type: 'uiCommand', id: PALETTE_COMMAND });
  await settle();
  return wrapper;
}

describe('the rows of the palette', () => {
  it('mounts at most fifty rows of a long list and names how many more match', async () => {
    const wrapper = await setup();
    const rows = wrapper.findAll('[role="option"]');
    expect(rows).toHaveLength(50);
    const total = Number(rows[0]?.attributes('aria-setsize'));
    expect(total).toBeGreaterThan(MANY);
    expect(wrapper.get('[data-testid="palette-more"]').text()).toContain(String(total - 50));
  });

  it('mounts the row the selection moved to, past the fiftieth', async () => {
    const wrapper = await setup();
    const field = wrapper.get('[role="combobox"]');
    await field.trigger('keydown', { key: 'ArrowUp' });
    await settle();
    const active = field.attributes('aria-activedescendant') ?? '';
    expect(active).not.toBe('');
    const row = wrapper.get(`[id="${active}"]`);
    // The highlight is on the last match, not back on the first row the list re-rendered with.
    expect(row.attributes('aria-posinset')).toBe(row.attributes('aria-setsize'));
  });

  it('mounts every row of a short list, without a note', async () => {
    const wrapper = await setup();
    await wrapper.get('[role="combobox"]').setValue('Row 11');
    await nextTick();
    const rows = wrapper.findAll('[role="option"]');
    expect(rows.length).toBe(Number(rows[0]?.attributes('aria-setsize')));
    expect(wrapper.find('[data-testid="palette-more"]').exists()).toBe(false);
  });
});
