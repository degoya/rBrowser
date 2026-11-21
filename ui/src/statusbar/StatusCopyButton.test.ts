import { afterEach, describe, expect, it, vi } from 'vitest';
import { mount, type VueWrapper } from '@vue/test-utils';
import { nextTick } from 'vue';
import ui from '@nuxt/ui/vue-plugin';
import { createShellI18n } from '../i18n';
import { IpcClient } from '../ipc/client';
import type { CoreToShell, ShellToCore } from '../ipc/protocol';
import StatusCopyButton from './StatusCopyButton.vue';

let current: VueWrapper | null = null;

async function flush(): Promise<void> {
  for (let i = 0; i < 6; i += 1) {
    await Promise.resolve();
    await nextTick();
  }
}

afterEach(() => {
  current?.unmount();
  current = null;
});

describe('the status bar copy button', () => {
  it('copies the report it names and shows a check until the message changes', async () => {
    const sent: ShellToCore[] = [];
    const client = new IpcClient({
      send: async (message): Promise<CoreToShell> => {
        sent.push(message);
        return { type: 'bugReport', answer: { kind: 'markdown', report: 'r1', text: '# Bug report\n' } };
      },
      subscribe: () => () => undefined,
    });
    const clipboard = { writeText: vi.fn(async () => undefined) };
    const wrapper = mount(StatusCopyButton, {
      props: { client, clipboard, copy: { labelKey: 'report.action.copy_markdown', report: 'r1' } },
      attachTo: document.body,
      global: { plugins: [ui, createShellI18n()] },
    });
    current = wrapper;
    const label = () => wrapper.get('[data-testid="status-copy"]').attributes('aria-label');
    const before = label();
    expect(before).toBeTruthy();
    expect(wrapper.get('[data-testid="status-copy"]').attributes('title')).toBe(before);
    await wrapper.get('[data-testid="status-copy"]').trigger('click');
    await flush();
    expect(sent).toEqual([{ type: 'bugReport', request: { op: 'getMarkdown', report: 'r1' } }]);
    expect(clipboard.writeText).toHaveBeenCalledWith('# Bug report\n');
    expect(label()).not.toBe(before);
    await wrapper.setProps({ copy: { labelKey: 'report.action.copy_markdown', report: 'r2' } });
    await flush();
    expect(label()).toBe(before);
  });
});
