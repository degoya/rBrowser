// The panel in a window of its own: the panels window is played by spying on postMessage and
// dispatching `message` events from window.parent (the test window itself). `npm test` in web/.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { mountPanel } from '../web-common/panel-ui/src/mount';
import App from './panel/App.vue';

type Posted = { type?: string; id?: number; tool?: string; args?: unknown };

let posted: Posted[] = [];
let unmount: () => void = () => undefined;

const STRINGS: Record<string, string> = {
  'plugin.example-feature.panels.main': 'Example feature',
  'plugin.example-feature.panel.count': '{n, plural, one {The grid has # view.} other {The grid has # views.}}',
  'plugin.example-feature.panel.empty': 'The grid has no views.',
  'plugin.example-feature.panel.loading': 'Counting the views...',
  'plugin.example-feature.panel.refresh': 'Count again',
  'plugin.example-feature.panel.width': '{width} px',
  'plugins.error.busy': 'The plugin is busy.',
};

const $ = (selector: string): HTMLElement | null => document.querySelector<HTMLElement>(selector);
const settle = async (): Promise<void> => {
  await new Promise((resolve) => setTimeout(resolve, 0));
  await nextTick();
};

async function fromHost(data: unknown): Promise<void> {
  window.dispatchEvent(new MessageEvent('message', { data, source: window }));
  await settle();
}

/** Answers the last `count_views` call. */
async function answer(result: { ok: true; value: unknown } | { ok: false; error: string }): Promise<void> {
  const call = posted.filter((message) => message.type === 'call' && message.tool === 'count_views').at(-1);
  await fromHost({ type: 'result', id: call?.id, ...result });
}

beforeEach(async () => {
  posted = [];
  document.body.innerHTML = '<div id="app"></div>';
  vi.spyOn(window, 'postMessage').mockImplementation((message: unknown) => {
    posted.push(message as Posted);
  });
  unmount = mountPanel(App, { prefix: 'plugin.example-feature.', name: 'example-feature panel' }).unmount;
  await fromHost({ type: 'settings', strings: STRINGS, settings: { 'ui.theme': 'light' } });
});

afterEach(() => {
  unmount();
  vi.restoreAllMocks();
});

describe('the example panel', () => {
  it('asks for its texts, then counts the views once they arrived', () => {
    expect(posted[0]).toEqual({ type: 'settings' });
    expect(posted.filter((message) => message.type === 'call')).toEqual([{ type: 'call', id: 1, tool: 'count_views', args: {} }]);
    expect($('#title')?.textContent).toBe('Example feature');
    expect($('#loading')?.textContent).toContain('Counting the views...');
  });

  it('shows the count and a badge per width', async () => {
    await answer({ ok: true, value: { count: 2, widths: [375, 1440] } });
    expect($('#loading')).toBeNull();
    expect($('#status')?.textContent).toContain('The grid has 2 views.');
    expect([...document.querySelectorAll('#widths [data-width]')].map((badge) => badge.textContent?.trim())).toEqual(['375 px', '1440 px']);
  });

  it('shows the empty state for a grid without views', async () => {
    await answer({ ok: true, value: { count: 0, widths: [] } });
    expect($('#empty')?.textContent).toContain('The grid has no views.');
  });

  it('counts again on Refresh and says a failure as a sentence', async () => {
    await answer({ ok: true, value: { count: 1, widths: [375] } });
    $('#refresh')?.click();
    await settle();
    await answer({ ok: false, error: 'plugins.error.busy' });
    expect($('#status')?.dataset.tone).toBe('error');
    expect($('#status')?.textContent).toContain('The plugin is busy.');
  });
});
