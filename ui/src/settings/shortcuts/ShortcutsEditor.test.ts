import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { createFakeCore } from '../test/fakeCore';
import { mountPage, settle } from '../test/mountPage';
import ShortcutsEditor from './ShortcutsEditor.vue';

let current: VueWrapper | null = null;

async function open(core = createFakeCore()) {
  const mounted = await mountPage(ShortcutsEditor, {}, core);
  current = mounted.wrapper;
  return mounted;
}

afterEach(() => {
  current?.unmount();
  current = null;
});

function key(code: string, mods: Partial<KeyboardEventInit> = {}): KeyboardEventInit {
  return { code, ...mods };
}

async function record(wrapper: VueWrapper, id: string, presses: KeyboardEventInit[]): Promise<void> {
  await wrapper.get(`[data-command="${id}"] [data-testid="record"]`).trigger('click');
  await settle();
  const recorder = wrapper.get(`[data-command="${id}"] [data-testid="recorder"]`);
  for (const press of presses) {
    await recorder.trigger('keydown', press);
  }
  await settle();
}

function transferField(wrapper: VueWrapper) {
  const field = wrapper.get('[data-testid="transfer"]');
  return field.element.tagName === 'TEXTAREA' ? field : field.get('textarea');
}

describe('the shortcut editor', () => {
  it('lists the commands by group with their bindings', async () => {
    const { wrapper } = await open();
    const row = wrapper.get('[data-command="sync.toggle"]');
    expect(row.findAll('[data-testid="binding"]').map((b) => b.text())).toEqual(['Ctrl+Shift+Space S S']);
    expect(wrapper.text()).toContain('Sync');
    expect(wrapper.text()).toContain('Reload');
  });

  it('shows one flat list on request', async () => {
    const { wrapper } = await open();
    expect(wrapper.findAll('[data-testid="group-title"]').length).toBeGreaterThan(1);
    await wrapper.get('[data-testid="flat"] button').trigger('click');
    await settle();
    expect(wrapper.findAll('[data-testid="group-title"]')).toHaveLength(0);
    expect(wrapper.findAll('[data-command]').length).toBeGreaterThan(1);
  });

  it('records a binding, checks it and saves it', async () => {
    const { wrapper, core } = await open();
    await record(wrapper, 'reload.all', [key('F9', { ctrlKey: true, shiftKey: true })]);
    expect(core.sent).toContainEqual({ type: 'checkBinding', id: 'reload.all', binding: 'Ctrl+Shift+F9' });
    expect(core.sent).toContainEqual({
      type: 'setBindings',
      id: 'reload.all',
      bindings: ['Ctrl+Shift+Space R R', 'Ctrl+Shift+F9'],
    });
    const bindings = wrapper.get('[data-command="reload.all"]').findAll('[data-testid="binding"]');
    expect(bindings.map((b) => b.text())).toContain('Ctrl+Shift+F9');
  });

  it('refuses a reserved combination with its reason', async () => {
    const { wrapper, core } = await open();
    await record(wrapper, 'reload.all', [key('KeyT', { ctrlKey: true })]);
    const refusal = wrapper.get('[data-command="reload.all"] [data-testid="refusal"]').text();
    expect(refusal).toContain('Ctrl+T');
    expect(refusal).toContain('new tab');
    expect(core.sent.some((m) => m.type === 'setBindings')).toBe(false);
  });

  it('asks replace, swap or cancel on a binding another command holds', async () => {
    const leaderSS = [key('Space', { ctrlKey: true, shiftKey: true }), key('KeyS'), key('KeyS')];
    const { wrapper, core } = await open();
    await record(wrapper, 'reload.all', leaderSS);
    const prompt = wrapper.get('[data-command="reload.all"] [data-testid="conflict"]');
    expect(prompt.attributes('role')).toBe('alertdialog');
    expect(prompt.text()).toContain('is bound to');
    expect(document.activeElement).toBe(prompt.get('[data-testid="conflict-replace"]').element);
    await prompt.get('[data-testid="conflict-cancel"]').trigger('click');
    await settle();
    expect(wrapper.find('[data-testid="conflict"]').exists()).toBe(false);
    expect(core.sent.some((m) => m.type === 'setBindings')).toBe(false);

    await record(wrapper, 'reload.all', leaderSS);
    await wrapper.get('[data-testid="conflict-swap"]').trigger('click');
    await settle();
    const sets = core.sent.filter((m) => m.type === 'setBindings');
    expect(sets).toEqual([
      { type: 'setBindings', id: 'sync.toggle', bindings: [] },
      { type: 'setBindings', id: 'reload.all', bindings: ['Ctrl+Shift+Space S S'] },
      { type: 'setBindings', id: 'sync.toggle', bindings: ['Ctrl+Shift+Space R R'] },
    ]);
  });

  it('replaces a held binding', async () => {
    const { wrapper, core } = await open();
    await record(wrapper, 'reload.all', [key('Digit2', { ctrlKey: true, shiftKey: true })]);
    await wrapper.get('[data-testid="conflict-replace"]').trigger('click');
    await settle();
    expect(core.sent.filter((m) => m.type === 'setBindings')).toEqual([
      { type: 'setBindings', id: 'view.set_breakpoint.2', bindings: ['Ctrl+Shift+Space B 2'] },
      { type: 'setBindings', id: 'reload.all', bindings: ['Ctrl+Shift+Space R R', 'Ctrl+Shift+2'] },
    ]);
  });

  it('finds the command on a pressed combination', async () => {
    const { wrapper } = await open();
    await wrapper.get('[data-testid="key-search"]').trigger('click');
    await settle();
    await wrapper.get('[data-testid="key-search-capture"]').trigger('keydown', key('Digit2', { ctrlKey: true, shiftKey: true }));
    await settle();
    expect(wrapper.findAll('[data-command]').map((row) => row.attributes('data-command'))).toEqual(['view.set_breakpoint.2']);
    expect(wrapper.get('[data-testid="key-search-active"]').text()).toContain('Ctrl+Shift+2');
    await wrapper.get('[data-testid="key-search-clear"]').trigger('click');
    await settle();
    expect(wrapper.findAll('[data-command]').length).toBeGreaterThan(1);
  });

  it('resets one group and keeps the others', async () => {
    const { wrapper, core } = await open();
    await record(wrapper, 'reload.all', [key('F9', { ctrlKey: true, shiftKey: true })]);
    const reset = wrapper.get('[data-group="reload"] [data-testid="reset-group"]');
    await reset.trigger('click');
    await settle();
    expect(core.sent).toContainEqual({ type: 'resetBinding', id: 'reload.all' });
    expect(wrapper.get('[data-group="sync"] [data-testid="reset-group"]').attributes('disabled')).toBeDefined();
  });

  it('marks a plugin default that stayed unbound', async () => {
    const { wrapper } = await open();
    const row = wrapper.get('[data-command="plugin.acme.close_all"]');
    expect(row.get('[data-testid="dropped"]').text()).toContain('Ctrl+W');
    expect(row.get('[data-testid="dropped"]').text()).toContain('leader');
  });

  it('exports and imports the same bindings, with a preview first', async () => {
    const { wrapper, core } = await open();
    await record(wrapper, 'reload.all', [key('F9', { ctrlKey: true, shiftKey: true })]);
    await wrapper.get('[data-testid="export"]').trigger('click');
    await settle();
    const exported = (transferField(wrapper).element as HTMLTextAreaElement).value;
    expect(JSON.parse(exported)).toEqual({ version: 1, bindings: { 'reload.all': ['Ctrl+Shift+Space R R', 'Ctrl+Shift+F9'] } });
    await wrapper.get('[data-testid="reset-all"]').trigger('click');
    await settle();
    await transferField(wrapper).setValue(exported);
    await wrapper.get('[data-testid="import"]').trigger('click');
    await settle();
    const preview = wrapper.get('[data-testid="import-preview"]');
    expect(preview.text()).toContain('Ctrl+Shift+F9');
    expect(core.sent.some((m) => m.type === 'importBindings')).toBe(false);
    await wrapper.get('[data-testid="import-apply"]').trigger('click');
    await settle();
    expect(core.sent.some((m) => m.type === 'importBindings')).toBe(true);
    const bindings = wrapper.get('[data-command="reload.all"]').findAll('[data-testid="binding"]');
    expect(bindings.map((b) => b.text())).toEqual(['Ctrl+Shift+Space R R', 'Ctrl+Shift+F9']);
  });

  it('shows the conflicts of an import before anything is saved, and refuses a bad file', async () => {
    const { wrapper, core } = await open();
    await transferField(wrapper).setValue(JSON.stringify({ version: 1, bindings: { 'reload.all': ['Ctrl+Shift+Space S S'] } }));
    await wrapper.get('[data-testid="import"]').trigger('click');
    await settle();
    expect(wrapper.get('[data-testid="import-conflicts"]').text()).toContain('Ctrl+Shift+Space S S');
    await wrapper.get('[data-testid="import-cancel"]').trigger('click');
    await settle();
    expect(wrapper.find('[data-testid="import-preview"]').exists()).toBe(false);
    expect(core.sent.some((m) => m.type === 'importBindings')).toBe(false);

    await transferField(wrapper).setValue('{"version":7,"bindings":{}}');
    await wrapper.get('[data-testid="import"]').trigger('click');
    await settle();
    expect(wrapper.get('[data-testid="import-error"]').attributes('role')).toBe('alert');
    expect(wrapper.find('[data-testid="import-preview"]').exists()).toBe(false);
  });

  it('is operable with the keyboard alone: every control is a native, focusable element', async () => {
    const { wrapper } = await open();
    const controls = wrapper.findAll('button, input, textarea');
    expect(controls.length).toBeGreaterThan(5);
    for (const control of controls) {
      // The group tabs rove: one is in the Tab order, the arrow keys reach the rest.
      if (control.attributes('role') !== 'tab') {
        expect(control.attributes('tabindex')).not.toBe('-1');
      }
    }
    for (const button of wrapper.findAll('button')) {
      const name = button.attributes('aria-label') ?? button.text();
      expect(name.trim()).not.toBe('');
    }
  });
});

describe('the titles of plugin commands (RB-080-82)', () => {
  it('shows a plugin command by the title from its catalogue, never the raw key', async () => {
    const { wrapper } = await open();
    const row = wrapper.get('[data-command="plugin.acme.close_all"]');
    expect(row.text()).toContain('Acme: Close every tab');
    expect(row.text()).not.toContain('plugin.acme.commands.close_all');
  });

  it('finds a plugin command by its title', async () => {
    const { wrapper } = await open();
    await wrapper.get('[data-testid="shortcut-search"]').setValue('close every');
    await settle();
    expect(wrapper.findAll('[data-command]').map((row) => row.attributes('data-command'))).toEqual([
      'plugin.acme.close_all',
    ]);
  });
});

describe('the command id on request (RB-080-70, RB-080-82)', () => {
  it('names a command by its title, the id in the tooltip; settings.showKeys prints it', async () => {
    const { wrapper } = await open();
    const row = () => wrapper.get('[data-command="sync.toggle"]');
    expect(row().find('[data-testid="command-id"]').exists()).toBe(false);
    expect(row().get('[data-testid="command-title"]').attributes('title')).toBe('sync.toggle');
    await wrapper.setProps({ showKeys: true });
    await settle();
    expect(row().get('[data-testid="command-id"]').text()).toBe('sync.toggle');
  });
});

describe('the bindings by leader group (RB-080-72)', () => {
  function tabs(wrapper: VueWrapper) {
    return wrapper.findAll('[data-testid="group-tabs"] button[role="tab"]');
  }

  function visibleGroups(wrapper: VueWrapper): (string | undefined)[] {
    return wrapper
      .findAll('[data-group]')
      .filter((group) => group.isVisible())
      .map((group) => group.attributes('data-group'));
  }

  it('shows one group per tab, the first open', async () => {
    const { wrapper } = await open();
    expect(tabs(wrapper).map((tab) => tab.text())).toEqual(
      wrapper.findAll('[data-group]').map((group) => group.get('[data-testid="group-title"]').text()),
    );
    expect(visibleGroups(wrapper)).toEqual(['sync']);
    await tabs(wrapper)[1]!.trigger('mousedown', { button: 0 });
    await settle();
    expect(visibleGroups(wrapper)).toEqual(['reload']);
  });

  it('keeps the fifteen group tabs in one row that scrolls, never wrapped (RB-080-81)', async () => {
    const { wrapper } = await open();
    const list = wrapper.get('[data-testid="group-tabs"] [role="tablist"]');
    expect(list.classes()).toContain('overflow-x-auto');
    expect(list.classes()).not.toContain('flex-wrap');
  });

  it('searches every group, the tabs hidden', async () => {
    const { wrapper } = await open();
    await wrapper.get('[data-testid="shortcut-search"]').setValue('Ctrl+Shift+Space');
    await settle();
    expect(wrapper.find('[data-testid="group-tabs"]').exists()).toBe(false);
    expect(visibleGroups(wrapper).length).toBeGreaterThan(1);
  });
});
