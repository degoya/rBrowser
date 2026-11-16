// RB-090-46: the palette's keyboard, mouse and screen-reader behaviour, pinned before it moved
// to Nuxt UI's `UCommandPalette` and held after. The selectors name no implementation: the field
// is the palette's text input, the highlighted row the one its `aria-activedescendant` names.
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

const MANY = 30;

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

/** Thirty plain rows in the group "Rows" and a step "Pick a row ..." with three of its own. */
function rowsProvider(): () => void {
  return registerPaletteProvider({
    id: 'keys-test',
    entries: (): PaletteEntry[] => [
      ...Array.from({ length: MANY }, (_, index): PaletteEntry => ({
        key: `row:${index}`,
        scope: 'command',
        title: `Row ${String(index).padStart(2, '0')}`,
        group: 'Rows',
        action: { kind: 'send', message: { type: 'runCommand', id: `row.${index}` } },
      })),
      {
        key: 'pick',
        scope: 'command',
        title: 'Pick a row ...',
        action: { kind: 'step', step: 'pick' },
      },
      ...['alpha', 'beta', 'gamma'].map((name): PaletteEntry => ({
        key: `pick:${name}`,
        scope: 'command',
        title: `Pick ${name}`,
        parent: 'pick',
        action: { kind: 'send', message: { type: 'runCommand', id: `pick.${name}` } },
      })),
    ],
  });
}

async function setup() {
  unregister = rowsProvider();
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
  const field = () => wrapper.get('[data-testid="palette"] input');
  const highlighted = () => {
    const id = field().attributes('aria-activedescendant') ?? '';
    expect(id).not.toBe('');
    return wrapper.get(`[id="${id}"]`);
  };
  const position = () => Number(highlighted().attributes('aria-posinset'));
  const total = () => Number(highlighted().attributes('aria-setsize'));
  const type = async (text: string): Promise<void> => {
    await field().setValue(text);
    await settle();
  };
  /** Dispatches a cancelable keydown on the field and says whether it was prevented. */
  const key = async (name: string, init: KeyboardEventInit = {}): Promise<boolean> => {
    const event = new KeyboardEvent('keydown', { key: name, bubbles: true, cancelable: true, ...init });
    field().element.dispatchEvent(event);
    await settle();
    return event.defaultPrevented;
  };
  const isOpen = () => wrapper.find('[data-testid="palette"]').exists();
  return { wrapper, core, field, highlighted, position, total, type, key, isOpen };
}

describe('the palette keyboard', () => {
  it('moves the selection by ten with Page Down and Page Up', async () => {
    const { type, key, position } = await setup();
    await type('row');
    expect(position()).toBe(1);
    expect(await key('PageDown')).toBe(true);
    expect(position()).toBe(11);
    await key('PageUp');
    expect(position()).toBe(1);
  });

  it('wraps from the first match to the last and back', async () => {
    const { type, key, position, total } = await setup();
    await type('row');
    await key('ArrowUp');
    expect(position()).toBe(total());
    await key('ArrowDown');
    expect(position()).toBe(1);
  });

  it('leaves Home and End to the field', async () => {
    const { type, key, position } = await setup();
    await type('row');
    await key('ArrowDown');
    expect(await key('Home')).toBe(false);
    expect(await key('End')).toBe(false);
    expect(position()).toBe(2);
  });

  it('keeps Escape from the page behind it', async () => {
    const { key } = await setup();
    let seen = false;
    const listener = (event: KeyboardEvent): void => {
      seen ||= event.key === 'Escape';
    };
    document.addEventListener('keydown', listener);
    await key('Escape');
    document.removeEventListener('keydown', listener);
    expect(seen).toBe(false);
  });

  it('goes back from a step with Backspace in the empty field, not while text is typed', async () => {
    const { wrapper, type, key, isOpen } = await setup();
    await type('pick a row');
    await key('Enter');
    expect(wrapper.get('[data-testid="palette-step"]').text()).toContain('Pick a row');
    await type('al');
    await key('Backspace');
    expect(wrapper.find('[data-testid="palette-step"]').exists()).toBe(true);
    await type('');
    await key('Backspace');
    expect(wrapper.find('[data-testid="palette-step"]').exists()).toBe(false);
    expect(isOpen()).toBe(true);
  });

  it('runs the entry of a step with Enter', async () => {
    const { type, key, core, isOpen } = await setup();
    await type('pick a row');
    await key('Enter');
    await type('gamma');
    await key('Enter');
    expect(core.sent).toContainEqual({ type: 'runCommand', id: 'pick.gamma' });
    expect(isOpen()).toBe(false);
  });

  it('does nothing on Ctrl+Enter for an entry without a command', async () => {
    const { type, key, core, isOpen } = await setup();
    await type('row 07');
    await key('Enter', { ctrlKey: true });
    expect(isOpen()).toBe(true);
    expect(core.sent.some((m) => m.type === 'openSettings' || m.type === 'runCommand')).toBe(false);
  });
});

describe('the palette rows', () => {
  it('names each row\'s group and its place in the list', async () => {
    const { wrapper, type, total } = await setup();
    await type('row');
    const rows = wrapper.findAll('[role="option"]');
    expect(rows[0]?.text()).toContain('Rows');
    expect(total()).toBe(rows.length);
    expect(rows.map((row) => Number(row.attributes('aria-posinset')))).toEqual(rows.map((_, index) => index + 1));
  });

  it('highlights the first match after every keystroke', async () => {
    const { type, key, position, highlighted } = await setup();
    await type('row');
    await key('ArrowDown');
    await key('ArrowDown');
    await type('row 1');
    expect(position()).toBe(1);
    expect(highlighted().text()).toContain('Row 1');
  });

  it('runs a row on a click without taking the focus from the field', async () => {
    const { wrapper, type, field, core, isOpen } = await setup();
    await type('row 05');
    const row = wrapper.findAll('[role="option"]').find((option) => option.text().includes('Row 05'));
    expect(row).toBeDefined();
    const press = new MouseEvent('mousedown', { bubbles: true, cancelable: true });
    row?.element.dispatchEvent(press);
    expect(press.defaultPrevented).toBe(true);
    expect(document.activeElement).toBe(field().element);
    await row?.trigger('click');
    await settle();
    expect(core.sent).toContainEqual({ type: 'runCommand', id: 'row.5' });
    expect(isOpen()).toBe(false);
  });

  it('opens a step on a click', async () => {
    const { wrapper, type } = await setup();
    await type('pick a row');
    const row = wrapper.findAll('[role="option"]').find((option) => option.text().includes('Pick a row'));
    await row?.trigger('click');
    await settle();
    expect(wrapper.get('[data-testid="palette-step"]').text()).toContain('Pick a row');
  });
});

// RB-090-46: Nuxt UI's list highlights the row under the pointer; the palette's selection follows.
describe('the palette under the pointer', () => {
  const hover = (row: { element: Element }): void => {
    row.element.dispatchEvent(new Event('pointermove', { bubbles: true }));
  };

  it('makes the row under the pointer the selection, and Enter runs it', async () => {
    const { wrapper, type, key, core, position, highlighted } = await setup();
    await type('row');
    const rows = wrapper.findAll('[role="option"]');
    hover(rows[4] as (typeof rows)[number]);
    await settle();
    expect(position()).toBe(5);
    expect(highlighted().attributes('data-highlighted')).toBeDefined();
    expect(wrapper.findAll('[role="option"][data-highlighted]')).toHaveLength(1);
    await key('ArrowDown');
    expect(position()).toBe(6);
    await key('Enter');
    expect(core.sent.some((m) => m.type === 'runCommand')).toBe(true);
  });

  it('puts the highlight back on the selection when the pointer leaves the list', async () => {
    const { wrapper, type, key, position } = await setup();
    await type('row');
    await key('ArrowDown');
    await key('ArrowDown');
    wrapper.get('[role="listbox"]').element.parentElement?.dispatchEvent(new Event('pointerleave'));
    await settle();
    expect(position()).toBe(3);
  });

  it('goes back from a step with its back button', async () => {
    const { wrapper, type, key, field, isOpen } = await setup();
    await type('pick a row');
    await key('Enter');
    const back = wrapper.get('[data-testid="palette-back"]');
    expect(back.attributes('aria-label')).toBe('Back to all commands (Backspace)');
    await back.trigger('click');
    await settle();
    expect(wrapper.find('[data-testid="palette-step"]').exists()).toBe(false);
    expect(isOpen()).toBe(true);
    expect(document.activeElement).toBe(field().element);
  });
});

describe('the palette for a screen reader', () => {
  it('is a labelled dialog whose field is a combobox over the list, with a hint', async () => {
    const { wrapper, field } = await setup();
    expect(wrapper.get('[data-testid="palette"]').attributes('role')).toBe('dialog');
    expect(wrapper.get('[data-testid="palette"]').attributes('aria-label')).toBe('Command palette');
    expect(field().attributes('role')).toBe('combobox');
    expect(field().attributes('aria-label')).toBe('Search commands, views and settings');
    const list = wrapper.get(`[id="${field().attributes('aria-controls')}"]`);
    expect(list.attributes('role')).toBe('listbox');
    expect(list.attributes('aria-label')).toBe('Matches');
    const hint = wrapper.get(`[id="${field().attributes('aria-describedby')}"]`);
    expect(hint.text()).toContain('Ctrl+Enter edits the shortcut');
  });

  it('says in its hint that Escape goes back while a step is open', async () => {
    const { wrapper, field, type, key } = await setup();
    await type('pick a row');
    await key('Enter');
    const hint = wrapper.get(`[id="${field().attributes('aria-describedby')}"]`);
    expect(hint.text()).toContain('Escape goes back');
  });

  it('announces the number of matches as it changes', async () => {
    const { wrapper, type, total } = await setup();
    await type('row 07');
    expect(wrapper.get('[role="status"]').attributes('aria-live')).toBe('polite');
    expect(wrapper.get('[role="status"]').text()).toBe('1 match');
    await type('row');
    expect(total()).toBeGreaterThanOrEqual(MANY);
    expect(wrapper.get('[role="status"]').text()).toBe(`${total()} matches`);
  });

  it('marks the highlighted row as the selected option', async () => {
    const { wrapper, type, key, highlighted } = await setup();
    await type('row');
    await key('ArrowDown');
    expect(highlighted().attributes('aria-selected')).toBe('true');
    expect(wrapper.findAll('[role="option"][aria-selected="true"]')).toHaveLength(1);
  });
});
