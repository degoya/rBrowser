// The panel in a window of its own: the calls it sends and how it renders the plugin's answers.
// The panels window is played by spying on postMessage and dispatching `message` events from
// window.parent (the test window itself). On Nuxt UI since RB-080-86.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { mountPanel } from '../../../web-common/panel-ui/src/mount';
import App from './panel/App.vue';

type Posted = { type: string; id?: number; tool?: string; command?: string; args?: Record<string, unknown> };

let posted: Posted[] = [];
let unmount: () => void = () => undefined;

const STRINGS: Record<string, string> = {
  'plugin.color-picker.panels.picker': 'Colour picker',
  'plugin.color-picker.panel.ratio': '{ratio}:1',
  'plugin.color-picker.panel.pass': 'Pass',
  'plugin.color-picker.panel.fail': 'Fail',
  'plugin.color-picker.panel.copy_format': 'Copy {format}: {value}',
  'plugin.color-picker.panel.copied': 'Copied {value}',
  'plugin.color-picker.panel.uses': '{count, plural, one {used once} other {used # times}}',
  'plugin.color-picker.panel.palette_read_done': '{count, plural, one {# colour} other {# colours}} in {view}.',
  'plugin.color-picker.panel.measure_hint': '{selector}: comfortable {min} to {max, plural, one {# character} other {# characters}} per line; lines above {warn} are marked.',
  'plugin.color-picker.panel.rating.long': 'too long',
  'plugin.color-picker.panel.rating.short': 'too short',
  'plugin.color-picker.panel.failed': 'The colour picker could not finish this.',
  'plugin.color-picker.errors.unsupported_engine': 'Chromium views only.',
  'plugin.color-picker.panel.pick': 'Pick from the page',
  'plugin.color-picker.panel.pick_active': 'Picking… Esc to stop',
  'plugin.color-picker.panel.picking': 'Picking.',
  'plugin.color-picker.panel.stopped': 'Stopped picking.',
  'plugin.color-picker.panel.picked': 'Took {value}.',
  'plugin.color-picker.panel.picked_managed': 'Took {value} as your display shows it.',
  'plugin.color-picker.panel.mark_uses': 'Mark the elements using {value}',
  'plugin.color-picker.panel.format.hex': 'HEX',
  'plugin.color-picker.panel.format.rgb': 'RGB',
};

function formats(hex: string, rgb: string) {
  return { hex, rgb, hsl: 'hsl()', oklch: 'oklch()' };
}

const RATING = {
  ratio: 4.54,
  normal: { aa: true, aaa: false },
  large: { aa: true, aaa: true },
  foreground: formats('#767676', 'rgb(118, 118, 118)'),
  background: formats('#ffffff', 'rgb(255, 255, 255)'),
};

const settle = async (): Promise<void> => {
  await new Promise((resolve) => setTimeout(resolve, 0));
  await nextTick();
};

async function host(data: unknown): Promise<void> {
  window.dispatchEvent(new MessageEvent('message', { data, source: window }));
  await settle();
}

/** The newest call of `name`. */
function last(name: string): Posted {
  const call = [...posted].reverse().find((m) => m.type === 'call' && (m.tool === name || m.command === name));
  expect(call, `a call of ${name}`).toBeDefined();
  return call as Posted;
}

async function answer(name: string, value: unknown, ok = true): Promise<void> {
  const call = last(name);
  await host(ok ? { type: 'result', id: call.id, ok: true, value } : { type: 'result', id: call.id, ok: false, error: value });
}

const $ = (id: string) => document.getElementById(id) as HTMLElement;
const status = (): HTMLElement => $('status');

beforeEach(async () => {
  posted = [];
  document.body.innerHTML = '<div id="app"></div>';
  vi.spyOn(window, 'postMessage').mockImplementation((message: unknown) => {
    posted.push(message as Posted);
  });
  unmount = mountPanel(App, { prefix: 'plugin.color-picker.', name: 'color-picker panel' }).unmount;
  await host({ type: 'settings', strings: STRINGS, settings: { 'color-picker.defaultFormat': 'rgb', 'color-picker.textMeasure.selector': 'p, li' } });
  await answer('contrast', RATING);
});

afterEach(() => {
  unmount();
  vi.restoreAllMocks();
});

describe('the colours and their contrast', () => {
  it('asks for the contrast of the starting pair and shows ratio and levels in words', () => {
    expect($('title').textContent).toBe('Colour picker');
    expect(last('contrast').args).toEqual({ foreground: '#000000', background: '#ffffff' });
    expect($('ratio').textContent).toBe('4.54:1');
    expect(document.querySelector('[data-size="normal"][data-level="aa"]')?.textContent).toBe('Pass');
    const fail = document.querySelector<HTMLElement>('[data-size="normal"][data-level="aaa"]');
    expect(fail?.textContent).toBe('Fail');
    expect(fail?.dataset.tone).toBe('error');
    expect(document.querySelector('[data-size="large"][data-level="aaa"]')?.textContent).toBe('Pass');
  });

  it('lists every format, the default format first, each with a copy button', async () => {
    const rows = document.querySelectorAll('[data-slot="foreground"] [data-formats] li');
    expect(rows).toHaveLength(4);
    expect((rows[0]?.querySelector('input') as HTMLInputElement).value).toBe('rgb(118, 118, 118)');
    const copy = rows[1]?.querySelector('button') as HTMLButtonElement;
    expect(copy.getAttribute('aria-label')).toBe('Copy HEX: #767676');
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
    copy.click();
    await settle();
    expect(writeText).toHaveBeenCalledWith('#767676');
    expect(status().textContent).toBe('Copied #767676');
  });

  it('takes a picked colour into its slot and swaps the two', async () => {
    await host({ type: 'event', message: { type: 'picked', slot: 'background', view: 2, color: formats('#1e88e5', 'rgb(30, 136, 229)') } });
    expect(last('contrast').args).toEqual({ foreground: '#000000', background: '#1e88e5' });
    $('swap').click();
    expect(last('contrast').args).toEqual({ foreground: '#1e88e5', background: '#000000' });
  });

  it('reads a colour typed in any format', async () => {
    const field = $('foreground-input') as HTMLInputElement;
    field.value = 'oklch(62.8% 0.25768 29.23)';
    field.dispatchEvent(new Event('input'));
    field.dispatchEvent(new Event('change'));
    await settle();
    expect(last('contrast').args?.foreground).toBe('oklch(62.8% 0.25768 29.23)');
  });

  it('starts the pipette through the command', () => {
    $('pick').click();
    expect(last('pick').command).toBe('pick');
  });
});

// RB-080-64: the button showed nothing until a colour was taken.
describe('the pick button while picking', () => {
  const button = () => $('pick') as HTMLButtonElement;
  const picks = () => posted.filter((m) => m.type === 'call' && m.command === 'pick');

  it('is the main action, pressed and saying how to stop at once, before the command answers', async () => {
    expect(button().hasAttribute('data-primary')).toBe(true);
    expect(button().getAttribute('aria-pressed')).toBe('false');
    expect(button().getAttribute('aria-describedby')).toBe('pick-hint');
    button().click();
    await nextTick();
    expect(button().getAttribute('aria-pressed')).toBe('true');
    expect(button().textContent).toBe('Picking… Esc to stop');
    expect(status().textContent).toBe('Picking.');
  });

  it('stops by a second click', async () => {
    button().click();
    await answer('pick', {});
    button().click();
    await nextTick();
    expect(picks()).toHaveLength(2);
    expect(picks()[1]?.args).toEqual({ stop: true });
    expect(button().getAttribute('aria-pressed')).toBe('false');
    expect(button().textContent).toBe('Pick from the page');
    expect(status().textContent).toBe('Stopped picking.');
  });

  it('stops by Esc in the panel', async () => {
    button().click();
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    await nextTick();
    expect(picks()[1]?.args).toEqual({ stop: true });
    expect(button().getAttribute('aria-pressed')).toBe('false');
  });

  it('shows the pipette started by its shortcut and its end in a view', async () => {
    await host({ type: 'event', message: { type: 'picking' } });
    expect(button().getAttribute('aria-pressed')).toBe('true');
    expect(picks()).toHaveLength(0);
    await host({ type: 'event', message: { type: 'stopped' } });
    expect(button().getAttribute('aria-pressed')).toBe('false');
    expect(status().textContent).toBe('Stopped picking.');
  });

  it('is released when a colour is taken', async () => {
    button().click();
    await host({ type: 'event', message: { type: 'picked', slot: 'foreground', view: 1, color: formats('#1e88e5', 'rgb(30, 136, 229)') } });
    expect(button().getAttribute('aria-pressed')).toBe('false');
    expect(status().textContent).toBe('Took #1e88e5.');
  });

  it("says a colour taken in the display's colours is no exact page colour", async () => {
    // RB-080-77: the window image of a display with a colour profile.
    await host({ type: 'event', message: { type: 'picked', slot: 'foreground', view: 1, managed: true, color: formats('#e746fb', 'rgb(231, 70, 251)') } });
    expect(status().textContent).toBe('Took #e746fb as your display shows it.');
  });

  it('is released when the command fails', async () => {
    button().click();
    await answer('pick', 'plugin.color-picker.errors.unsupported_engine', false);
    expect(button().getAttribute('aria-pressed')).toBe('false');
    // The code's own sentence (the wave's contract: never `{code}`).
    expect(status().textContent).toBe('Chromium views only.');
    expect(status().dataset.tone).toBe('error');
  });

  it('takes no Esc while idle', () => {
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    expect(picks()).toHaveLength(0);
  });
});

describe('the palette', () => {
  it('lists the active view colours, a count as a plural, and marks one colour uses', async () => {
    expect($('palette-empty')).not.toBeNull();
    $('palette-read').click();
    await answer('palette', {
      views: [
        {
          view: 1,
          label: 'Desktop',
          colors: [
            { hex: '#212121', count: 7, members: ['rgb(33, 33, 33)'] },
            { hex: '#fafafa', count: 1, members: ['rgb(250, 250, 250)'] },
          ],
        },
      ],
    });
    const rows = document.querySelectorAll('#palette li');
    expect(rows).toHaveLength(2);
    expect(rows[0]?.querySelector('[data-uses]')?.textContent).toBe('used 7 times');
    expect(rows[1]?.querySelector('[data-uses]')?.textContent).toBe('used once');
    expect(status().textContent).toBe('2 colours in Desktop.');
    const mark = rows[0]?.querySelector('button') as HTMLButtonElement;
    expect(mark.getAttribute('aria-label')).toBe('Mark the elements using #212121');
    mark.click();
    expect(last('palette').args).toEqual({ mark: ['rgb(33, 33, 33)'] });
    (rows[1]?.querySelector('[data-use="background"]') as HTMLButtonElement).click();
    expect(last('contrast').args).toEqual({ foreground: '#000000', background: '#fafafa' });
    $('palette-clear').click();
    expect(last('palette').args).toEqual({ mark: [] });
  });
});

describe('the text measure', () => {
  // The panel's frame has no `allow-forms`: a form never submits there (found by RB-080-66).
  it('has no form; the button and Enter in the field measure', async () => {
    expect(document.querySelector('form')).toBeNull();
    expect(($('measure-run') as HTMLButtonElement).type).toBe('button');
    const field = $('selector') as HTMLInputElement;
    field.value = 'article p';
    field.dispatchEvent(new Event('input'));
    await nextTick();
    field.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }));
    expect(last('measure_text').args).toEqual({ selector: 'article p' });
  });

  it('measures the setting selector and shows each view rating in words', async () => {
    expect(($('selector') as HTMLInputElement).value).toBe('p, li');
    $('measure-run').click();
    expect(last('measure_text').args).toEqual({ selector: 'p, li' });
    await answer('measure_text', {
      selector: 'p, li',
      optimal: { min: 45, max: 75 },
      warnAbove: 80,
      views: [
        { view: 1, label: 'Mobile', lines: 17, average: 38, max: 40, over: 0, rating: 'short' },
        { view: 2, label: 'Desktop', lines: 5, average: 151, max: 151, over: 4, rating: 'long' },
        { view: 3, label: 'Firefox', error: 'plugin.color-picker.errors.unsupported_engine' },
      ],
    });
    const rows = document.querySelectorAll('#measure-table tbody tr');
    expect(rows).toHaveLength(3);
    expect(rows[1]?.textContent).toContain('too long');
    expect(rows[1]?.querySelector<HTMLElement>('[data-rating]')?.dataset.tone).toBe('error');
    expect(rows[2]?.textContent).toContain('Chromium views only.');
    expect($('measure-hint').textContent).toBe('p, li: comfortable 45 to 75 characters per line; lines above 80 are marked.');
  });

  it('keeps a selector the person typed over the setting', async () => {
    const field = $('selector') as HTMLInputElement;
    field.value = 'main p';
    field.dispatchEvent(new Event('input'));
    await host({ type: 'settings', strings: STRINGS, settings: { 'color-picker.textMeasure.selector': 'h1' } });
    expect(($('selector') as HTMLInputElement).value).toBe('main p');
  });

  it('takes a measurement started in a view and asks the views for a choice', async () => {
    expect(document.getElementById('measure-table')).toBeNull();
    await host({ type: 'event', message: { type: 'measured', result: { selector: '#x', views: [{ view: 1, label: 'M', lines: 1, average: 50, max: 50, over: 0, rating: 'optimal' }] } } });
    expect(document.querySelectorAll('#measure-table tbody tr')).toHaveLength(1);
    $('measure-choose').click();
    expect(last('measure_text').args).toEqual({ choose: true });
    $('measure-clear').click();
    expect(last('measure_text').args).toEqual({ clear: true });
  });
});

describe('the bridge', () => {
  it('ignores messages from anyone but the panels window and shows failures', async () => {
    window.dispatchEvent(new MessageEvent('message', { data: { type: 'settings', strings: {} }, source: null }));
    await settle();
    expect($('ratio').textContent).toBe('4.54:1');
    $('palette-read').click();
    await answer('palette', 'plugins.error.busy', false);
    // A code without a text shows the panel's sentence, never the code (red before: "Error:
    // plugins.error.busy").
    expect(status().textContent).toBe('The colour picker could not finish this.');
    expect(status().dataset.tone).toBe('error');
  });

  it("takes the app's theme from its settings", async () => {
    await host({ type: 'settings', strings: STRINGS, settings: { 'ui.theme': 'dark' } });
    expect(document.documentElement.classList.contains('dark')).toBe(true);
  });
});
