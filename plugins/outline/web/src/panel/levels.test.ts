// RB-091-06: the panel shows a finding at the severity the plugin's `get` answers - the person's
// `checks.rules` level, as the findings panel shows it - not the rule's own; a rule turned off
// is not in the answer and shows nowhere.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { mountPanel } from '../../../../web-common/panel-ui/src/mount';
import en from '../../../locales/en.json';
import App from './App.vue';
import type { Answer } from './model';

type Posted = { type?: string; id?: number; tool?: string };

function flatten(value: unknown, prefix = ''): Record<string, string> {
  if (typeof value === 'string') return { [prefix]: value };
  return Object.entries(value as Record<string, unknown>).reduce<Record<string, string>>(
    (all, [key, child]) => ({ ...all, ...flatten(child, prefix ? `${prefix}.${key}` : key) }),
    {},
  );
}

const cell = (path: string) => ({ index: 0, visible: true, hidden: false, selector: `#${path}`, path });

/** The rule `heading-skip` (a warning of its own) raised to an error by `checks.rules`. */
const RAISED: Answer = {
  views: [{ id: 1, label: 'Desktop' }],
  findings: [
    {
      rule: 'heading-skip',
      severity: 'error',
      message: 'plugin.outline.findings.heading-skip',
      params: { from: 'h1', to: 'h3' },
      view: 1,
      path: 'h3',
    },
  ],
  sections: {
    headings: {
      rows: [
        { item: { level: 1, text: 'Title' }, views: { '1': cell('h1') } },
        { item: { level: 3, text: 'Deep' }, views: { '1': cell('h3') } },
      ],
    },
  },
};

let posted: Posted[] = [];
let unmount: () => void = () => undefined;

const settle = async (): Promise<void> => {
  await new Promise((resolve) => setTimeout(resolve, 0));
  await nextTick();
};

async function fromHost(data: unknown): Promise<void> {
  window.dispatchEvent(new MessageEvent('message', { data, source: window }));
  await settle();
}

async function answerGet(value: Answer): Promise<void> {
  for (const message of posted.filter((m) => m.type === 'call' && m.tool === 'get')) {
    await fromHost({ type: 'result', id: message.id, ok: true, value });
  }
}

const rows = (): HTMLElement[] => [...document.querySelectorAll<HTMLElement>('#tree [role="treeitem"]')];

beforeEach(async () => {
  posted = [];
  document.body.innerHTML = '<div id="app"></div>';
  vi.spyOn(window, 'postMessage').mockImplementation((message: unknown) => {
    posted.push(message as Posted);
  });
  unmount = mountPanel(App, { prefix: 'plugin.outline.', name: 'outline panel' }).unmount;
  await fromHost({ type: 'settings', strings: flatten(en), settings: { 'plugins.panelMessageMaxBytes': 1024 * 1024 } });
});

afterEach(() => {
  unmount();
  vi.restoreAllMocks();
});

describe('the Outline panel with the rule levels of checks.rules', () => {
  it('shows a finding at the level the answer carries, not the rule\'s own', async () => {
    await answerGet(RAISED);
    const note = rows()[1]?.querySelector('[data-note]');
    expect(note?.getAttribute('data-severity')).toBe('error');
    expect(note?.textContent).toContain('h1');
  });

  it('shows nothing of a rule turned off', async () => {
    await answerGet({ ...RAISED, findings: [] });
    expect(rows()).toHaveLength(2);
    expect(document.querySelectorAll('#tree [data-note]')).toHaveLength(0);
  });
});
