// The overlay's colours (RB-090-58): every drawing carries them, the page script draws in them,
// and a change of the plugin's settings (the host's post `{"$settings": true}`) redraws at once.
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { drawing, fakePlugin, runPageScript, settle, type FakePlugin, type Message } from './support';

const COLORS = {
  ruler: '#202020e0',
  ticks: '#ffffff',
  guide: '#ff00ff',
  columnFill: '#00ff0033',
  columnEdge: '#00ff00',
  baseline: '#ff000080',
};

let plugin: FakePlugin;
let layout: Record<string, unknown>;

async function start(first: Record<string, unknown>): Promise<ShadowRoot> {
  layout = first;
  plugin = fakePlugin((message: Message) =>
    message.op === 'init'
      ? { state: {}, overrides: { visible: {} }, prefs: { unit: 'px', snapPx: 8 }, layout }
      : { layout },
  );
  const root = runPageScript(plugin);
  await settle();
  return root;
}

/** The custom properties the page script set for the colours, by name. */
function colorVars(root: ShadowRoot): Record<string, string> {
  const text = root.querySelector('style.colors')?.textContent ?? '';
  return Object.fromEntries([...text.matchAll(/(--[a-z-]+):\s*([^;]+);/g)].map((match) => [match[1], match[2]?.trim()]));
}

beforeEach(() => {
  document.documentElement.querySelectorAll('[data-rbrowser-plugin]').forEach((node) => node.remove());
});

afterEach(() => {
  plugin?.stop();
});

describe('the overlay colours', () => {
  it('draw the ruler, ticks, guides, columns and baseline in the drawing colours', async () => {
    const root = await start(drawing({ colors: COLORS }));
    expect(colorVars(root)).toEqual({
      '--ruler': '#202020e0',
      '--tick': '#ffffff',
      '--guide': '#ff00ff',
      '--column': '#00ff0033',
      '--column-edge': '#00ff00',
      '--baseline': '#ff000080',
    });
    // The stylesheet uses them: the bar, the guides, both column edges, the grid lines.
    const sheet = root.querySelector('style')?.textContent ?? '';
    expect(sheet).toMatch(/\.ruler \{[^}]*background-color: var\(--ruler\)/);
    expect(sheet).toMatch(/\.columns div \{[^}]*border-left: 1px solid var\(--column-edge\)/);
    expect(sheet).toMatch(/\.columns div \{[^}]*border-right: 1px solid var\(--column-edge\)/);
    expect(sheet).toMatch(/\.guide \{[^}]*background: var\(--guide\)/);
    expect(sheet).toMatch(/\.baseline \{[^}]*var\(--baseline\)/);
  });

  it("keeps the stylesheet's colours without colours in the drawing", async () => {
    const root = await start(drawing());
    expect(colorVars(root)).toEqual({});
    const sheet = root.querySelector('style')?.textContent ?? '';
    // The defaults: the colours drawn before they could be set.
    expect(sheet).toContain('--ruler: #f6f7f9f0');
    expect(sheet).toContain('--guide: #00a2ff');
    expect(sheet).toContain('--column: #ff40811f');
    expect(sheet).toContain('--column-edge: #ff408100');
    expect(sheet).toContain('--baseline: #00a2ff59');
  });

  it('takes nothing into its stylesheet that is no hex colour', async () => {
    const root = await start(
      drawing({ colors: { ...COLORS, guide: 'red; } .ruler { display: none', ruler: 'url(https://x.test/a)', ticks: 7 } }),
    );
    const vars = colorVars(root);
    expect(vars['--guide']).toBeUndefined();
    expect(vars['--ruler']).toBeUndefined();
    expect(vars['--tick']).toBeUndefined();
    expect(vars['--baseline']).toBe('#ff000080');
    expect(root.querySelector('style.colors')?.textContent).not.toContain('display');
  });

  it('follows a colour changed in another view at once', async () => {
    const root = await start(drawing());
    layout = drawing({ colors: { ...COLORS, guide: '#123456' } });
    plugin.post({ state: { visible: { rulers: true } }, overrides: { colors: { guide: '#123456' } } });
    await settle();
    expect(colorVars(root)['--guide']).toBe('#123456');
  });

  it('asks for its drawing again when the settings changed, shown or not', async () => {
    const root = await start(drawing({ visible: { rulers: false, columns: false, baseline: false } }));
    const before = plugin.sent.length;
    layout = drawing({ colors: COLORS });
    plugin.post({ $settings: true });
    await settle();
    expect(plugin.sent.length).toBe(before + 1);
    expect(plugin.sent.at(-1)).toMatchObject({ op: 'layout' });
    expect(colorVars(root)['--ruler']).toBe('#202020e0');
  });
});
