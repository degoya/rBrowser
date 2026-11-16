import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import type { ProjectShortcut } from '../../ipc/protocol';
import { createFakeCore } from '../test/fakeCore';
import { mountPage, settle } from '../test/mountPage';
import ShortcutsEditor from './ShortcutsEditor.vue';

let current: VueWrapper | null = null;

afterEach(() => {
  current?.unmount();
  current = null;
});

async function open(projectShortcuts: ProjectShortcut[]) {
  const mounted = await mountPage(ShortcutsEditor, {}, createFakeCore({ projectShortcuts }));
  current = mounted.wrapper;
  return mounted;
}

describe("the shortcut editor's project shortcuts (RB-010-16)", () => {
  it('shows a conflict line for a reserved, a doubled and a taken project shortcut', async () => {
    const { wrapper, core } = await open([
      { breakpoint: 'mobile', binding: 'Ctrl+Shift+F9' },
      { breakpoint: 'tablet', binding: 'Alt+M', reasonKey: 'shortcuts.reserved.accesskey' },
      { breakpoint: 'wide', binding: 'Ctrl+Shift+F10', reasonKey: 'shortcuts.rejected.conflict', other: 'narrow' },
      { breakpoint: 'narrow', binding: 'Ctrl+Shift+F10', reasonKey: 'shortcuts.rejected.conflict', other: 'wide' },
      { breakpoint: 'watch', binding: 'Ctrl+Shift+Space R R', reasonKey: 'sizes.conflict.chord' },
      { breakpoint: 'tv', binding: 'Ctrl+Shift+2', reasonKey: 'shortcuts.rejected.conflict', other: 'view.set_breakpoint.2' },
    ]);
    expect(core.sent).toContainEqual({ type: 'listProjectShortcuts' });
    const rows = wrapper.findAll('[data-testid="project-shortcut"]');
    expect(rows.map((row) => row.attributes('data-breakpoint'))).toEqual(['mobile', 'tablet', 'wide', 'narrow', 'watch', 'tv']);
    expect(rows[0]?.attributes('data-conflict')).toBeUndefined();
    expect(rows[0]?.text()).toContain('Ctrl+Shift+F9');
    expect(rows[0]?.text()).toContain('In force');
    expect(rows[1]?.attributes('data-conflict')).toBe('true');
    expect(rows[1]?.text()).toContain('Alt+M');
    expect(rows[1]?.text()).toContain('Page accesskey and browser menus');
    expect(rows[2]?.text()).toContain('narrow');
    // A command is named by its title.
    expect(rows[5]?.text()).not.toContain('view.set_breakpoint.2');
    expect(rows[5]?.find('[role="alert"], [data-testid="project-conflict"]').exists()).toBe(true);
  });

  it('reads them again after a binding changed', async () => {
    const { wrapper, core } = await open([{ breakpoint: 'mobile', binding: 'Ctrl+Shift+F9' }]);
    const asked = () => core.sent.filter((m) => m.type === 'listProjectShortcuts').length;
    const before = asked();
    await wrapper.get('[data-command="reload.all"] [data-testid="record"]').trigger('click');
    await settle();
    await wrapper
      .get('[data-command="reload.all"] [data-testid="recorder"]')
      .trigger('keydown', { code: 'F8', ctrlKey: true, shiftKey: true });
    await settle();
    expect(core.sent).toContainEqual({ type: 'setBindings', id: 'reload.all', bindings: ['Ctrl+Shift+Space R R', 'Ctrl+Shift+F8'] });
    expect(asked()).toBeGreaterThan(before);
  });

  it('shows nothing without project shortcuts', async () => {
    const { wrapper } = await open([]);
    expect(wrapper.find('[data-testid="project-shortcuts"]').exists()).toBe(false);
  });
});
