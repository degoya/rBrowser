import UDropdownMenu, { type DropdownMenuItem } from '@nuxt/ui/components/DropdownMenu.vue';
import ui from '@nuxt/ui/vue-plugin';
import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import { createShellI18n } from '../i18n';
import { settle } from '../test/mountShell';
import FindingsExport from './FindingsExport.vue';
import FindingsPanel from './FindingsPanel.vue';
import { fakeChannel, sampleMessages, sampleRun } from './testing';

function mountExport(props: { disabled?: boolean; shortcut?: string | null } = {}) {
  return mount(FindingsExport, {
    props,
    attachTo: document.body,
    global: { plugins: [ui, createShellI18n()] },
  });
}

function menuItems(wrapper: ReturnType<typeof mountExport>): DropdownMenuItem[] {
  return (wrapper.getComponent(UDropdownMenu).props() as Record<string, unknown>).items as DropdownMenuItem[];
}

describe('FindingsExport', () => {
  it('exports in the format of the setting from the button', async () => {
    const wrapper = mountExport();
    await wrapper.find('[data-testid="export-run"]').trigger('click');
    expect(wrapper.emitted('export')).toEqual([[null]]);
    wrapper.unmount();
  });

  it('offers every format from the arrow, by name', () => {
    const wrapper = mountExport();
    const items = menuItems(wrapper);
    expect(items.map((item) => item.label)).toEqual(['SARIF', 'JUnit XML', 'HTML', 'JSON']);
    items[1]?.onSelect?.(new Event('select'));
    expect(wrapper.emitted('export')).toEqual([['junit']]);
    wrapper.unmount();
  });

  it('names both halves and the shortcut in the tooltip', () => {
    const wrapper = mountExport({ shortcut: 'Ctrl+Shift+Space K B' });
    const run = wrapper.find('[data-testid="export-run"]');
    expect(run.attributes('aria-label')).toBe('Export the findings');
    expect(run.attributes('title')).toContain('Ctrl+Shift+Space K B');
    expect(wrapper.find('[data-testid="export-menu"]').attributes('aria-label')).toBe('Export as');
    wrapper.unmount();
  });

  it('is off while checks run', () => {
    const wrapper = mountExport({ disabled: true });
    expect(wrapper.find('[data-testid="export-run"]').attributes('disabled')).toBeDefined();
    expect(wrapper.find('[data-testid="export-menu"]').attributes('disabled')).toBeDefined();
    wrapper.unmount();
  });
});

describe('FindingsPanel export', () => {
  function mountPanel() {
    const core = fakeChannel();
    const wrapper = mount(FindingsPanel, {
      props: { channel: core.channel },
      attachTo: document.body,
      global: { plugins: [ui, createShellI18n()] },
    });
    return { core, wrapper };
  }

  it('shows no export before a run', () => {
    const { wrapper } = mountPanel();
    expect(wrapper.find('[data-testid="export"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it('runs checks.export on the last run, with the chosen format', async () => {
    const { core, wrapper } = mountPanel();
    core.emit({ type: 'findings', run: sampleRun(), messages: sampleMessages });
    await settle();
    await wrapper.find('[data-testid="export-run"]').trigger('click');
    await settle();
    wrapper.findComponent(FindingsExport).vm.$emit('export', 'html');
    await settle();
    expect(core.sent).toEqual([
      { type: 'runCommand', id: 'checks.export' },
      { type: 'runCommand', id: 'checks.export', args: { format: 'html' } },
    ]);
    wrapper.unmount();
  });
});
