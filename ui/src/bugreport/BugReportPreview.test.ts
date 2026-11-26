import { afterEach, describe, expect, it } from 'vitest';
import { mount, type VueWrapper } from '@vue/test-utils';
import { nextTick } from 'vue';
import ui from '@nuxt/ui/vue-plugin';
import { createShellI18n } from '../i18n';
import { IpcClient } from '../ipc/client';
import type { CoreToShell, ShellToCore } from '../ipc/protocol';
import BugReportPreview from './BugReportPreview.vue';
import { PAGES } from '../settings/routes';
import { unnamedFocusables } from '../test/keyboard';

let current: VueWrapper | null = null;

async function flush(): Promise<void> {
  for (let i = 0; i < 6; i += 1) {
    await Promise.resolve();
    await nextTick();
  }
}

const HOSTILE = '<img src=x onerror="window.hacked=1">';

/** The page over a core with a report `waiting`; `refuse` makes the core refuse that step. */
async function setup(waiting: boolean, refuse?: 'getPreview' | 'decide') {
  const sent: ShellToCore[] = [];
  const client = new IpcClient({
    send: async (message): Promise<CoreToShell> => {
      sent.push(message);
      if (message.type === 'bugReport' && message.request.op === refuse) {
        throw new Error('refused');
      }
      if (waiting && message.type === 'bugReport' && message.request.op === 'getPreview') {
        return {
          type: 'bugReport',
          answer: {
            kind: 'preview',
            previewId: 9,
            folder: 'C:/work/shop/.rbrowser/out/reports/r',
            markdown: `# Bug report\n\n\`${HOSTILE}\`\n`,
            images: 2,
            annotate: true,
          },
        };
      }
      return { type: 'ack' };
    },
    subscribe: () => () => undefined,
  });
  const wrapper = mount(BugReportPreview, {
    props: { client },
    attachTo: document.body,
    global: { plugins: [ui, createShellI18n()] },
  });
  current = wrapper;
  await flush();
  return { wrapper, sent };
}

afterEach(() => {
  current?.unmount();
  current = null;
});

describe('the bug report preview page', () => {
  it('is the page of the route bug-report', async () => {
    expect((await PAGES['bug-report']!()).default).toBe(BugReportPreview);
  });

  it('shows the masked Markdown as plain text and saves it with Save', async () => {
    const { wrapper, sent } = await setup(true);
    const markdown = wrapper.get('[data-testid="bug-report-markdown"]');
    expect(markdown.text()).toContain(HOSTILE);
    // RB-090-08: the scrolling Markdown is a Tab stop and so a named region.
    expect(markdown.attributes('role')).toBe('region');
    expect(unnamedFocusables(wrapper.element)).toEqual([]);
    expect(markdown.find('img').exists()).toBe(false);
    expect(wrapper.find('[data-testid="bug-report-annotate"]').exists()).toBe(true);
    expect(wrapper.get('[data-testid="bug-report-folder"]').attributes('title')).toBe(
      'C:/work/shop/.rbrowser/out/reports/r',
    );
    await wrapper.get('[data-testid="bug-report-save"]').trigger('click');
    await flush();
    expect(sent.at(-1)).toEqual({ type: 'bugReport', request: { op: 'decide', previewId: 9, save: true } });
    expect(wrapper.find('[data-testid="bug-report-markdown"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="bug-report-decided"]').exists()).toBe(true);
  });

  it('drops the report with Escape', async () => {
    const { wrapper, sent } = await setup(true);
    await wrapper.get('[data-testid="bug-report-preview"]').trigger('keydown', { key: 'Escape' });
    await flush();
    expect(sent.at(-1)).toEqual({ type: 'bugReport', request: { op: 'decide', previewId: 9, save: false } });
  });

  it('says a failure at once, when it loads and when it saves (RB-099-21)', async () => {
    const failed = await setup(true, 'getPreview');
    expect(failed.wrapper.get('[data-testid="bug-report-failed"]').attributes('role')).toBe('alert');
    failed.wrapper.unmount();
    const { wrapper } = await setup(true, 'decide');
    await wrapper.get('[data-testid="bug-report-save"]').trigger('click');
    await flush();
    expect(wrapper.get('[data-testid="bug-report-refused"]').attributes('role')).toBe('alert');
  });

  it('says so when no report waits, with no buttons', async () => {
    const { wrapper } = await setup(false);
    expect(wrapper.find('[data-testid="bug-report-none"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="bug-report-save"]').exists()).toBe(false);
  });
});
