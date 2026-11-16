import { describe, expect, it, vi } from 'vitest';
import { provideMenuSource } from './nativeMenu';
import { nameOf, overflowEntries, sections } from './overflowEntries';

const TEXTS = { on: (label: string) => `${label}: on`, off: (label: string) => `${label}: off` };

function group(html: string): HTMLElement {
  const root = document.createElement('div');
  root.innerHTML = html;
  document.body.append(root);
  return root;
}

describe('overflowEntries', () => {
  it('turns a button into an entry that clicks it', () => {
    const root = group('<button aria-label="Settings"></button><button title="Zoom" disabled></button>');
    const clicked = vi.fn();
    root.querySelector('button')?.addEventListener('click', clicked);
    const entries = overflowEntries([['settings', root]], TEXTS);
    expect(entries.map((entry) => [entry.label, entry.disabled, entry.heading])).toEqual([
      ['Settings', false, null],
      ['Zoom', true, null],
    ]);
    entries[0]?.run();
    expect(clicked).toHaveBeenCalledOnce();
  });

  it('names the state of a switch and of a pressed toggle', () => {
    const root = group(
      '<button id="s1" role="switch" aria-checked="true"></button><label for="s1">Sync</label>' +
        '<button role="switch" aria-checked="false" aria-label="Hover"></button>' +
        '<button aria-pressed="true" aria-label="Grid"></button>' +
        '<button aria-pressed="false" aria-label="Canvas"></button>',
    );
    expect(overflowEntries([['sync', root]], TEXTS).map((entry) => entry.label)).toEqual([
      'Sync: on',
      'Hover: off',
      'Grid: on',
      'Canvas',
    ]);
  });

  // RB-090-41, RB-090-45: a pulldown drawn as the core's native menu is a button; its rows come
  // from the pulldown itself, a headed block, the one in effect checked as in its own menu.
  it('lists the rows of a native-menu pulldown under its name and chooses through it', () => {
    const root = group('<span><button aria-label="Theme: Dark" aria-haspopup="menu"></button></span>');
    const button = root.querySelector('button') as HTMLButtonElement;
    const chosen: string[] = [];
    provideMenuSource(button, {
      rows: () => [
        { kind: 'heading', label: 'Shop' },
        { kind: 'item', id: 'cart', label: 'Cart' },
        { kind: 'separator' },
        { kind: 'item', id: 'light', label: 'Light', checked: false },
        { kind: 'item', id: 'dark', label: 'Dark', checked: true },
        { kind: 'item', id: 'none', label: 'No URL', disabled: true },
      ],
      choose: (id) => chosen.push(id),
    });
    const entries = overflowEntries([['theme', root]], TEXTS);
    expect(
      entries.map((entry) => [entry.label, entry.disabled, entry.checked, entry.heading, entry.opens]),
    ).toEqual([
      ['Shop / Cart', false, undefined, 'Theme: Dark', false],
      ['Light', false, false, 'Theme: Dark', false],
      ['Dark', false, true, 'Theme: Dark', false],
      ['No URL', true, undefined, 'Theme: Dark', false],
    ]);
    entries[1]?.run();
    expect(chosen).toEqual(['light']);
    expect(new Set(entries.map((entry) => entry.key)).size).toBe(4);
  });

  it('knows no select any more: the toolbar has none (RB-090-45)', () => {
    const root = group('<select title="Old"><option value="a">A</option></select>');
    expect(overflowEntries([['bookmarks', root]], TEXTS)).toEqual([]);
  });

  it('names what is only to look at, disabled', () => {
    const root = group('<svg tabindex="0" aria-label="Canvas minimap"></svg>');
    expect(overflowEntries([['minimap-end', root]], TEXTS)).toMatchObject([
      { label: 'Canvas minimap', disabled: true },
    ]);
  });

  it('puts a notice before the button that answers it', () => {
    const root = group(
      '<div role="status" title="Browsersync on shop.test paused sync"><span>Sync paused</span>' +
        '<button>Sync anyway</button></div>',
    );
    expect(overflowEntries([['browsersync', root]], TEXTS).map((entry) => [entry.label, entry.disabled])).toEqual([
      ['Browsersync on shop.test paused sync', true],
      ['Sync anyway', false],
    ]);
    const plain = group('<div role="status"><span>Sync paused</span><button>Sync anyway</button></div>');
    expect(overflowEntries([['browsersync', plain]], TEXTS)[0]?.label).toBe('Sync paused');
  });

  it('marks a control that opens something beside itself', () => {
    const root = group(
      '<button aria-label="Extras" aria-expanded="false"></button>' +
        '<button aria-label="Add view" aria-haspopup="menu"></button><button aria-label="Reset"></button>',
    );
    expect(overflowEntries([['emulation', root]], TEXTS).map((entry) => [entry.group, entry.opens])).toEqual([
      ['emulation', true],
      ['emulation', false],
      ['emulation', false],
    ]);
  });

  it('gives a group without controls no entry', () => {
    expect(overflowEntries([['trust', group('')]], TEXTS)).toEqual([]);
  });
});

describe('sections', () => {
  it('keeps runs of one heading together', () => {
    const entry = (key: string, heading: string | null) => ({
      key,
      label: key,
      disabled: false,
      heading,
      group: 'g',
      opens: false,
      run: () => undefined,
    });
    const out = sections([entry('a', null), entry('b', 'Env'), entry('c', 'Env'), entry('d', null)]);
    expect(out.map((block) => [block.heading, block.entries.map((e) => e.key)])).toEqual([
      [null, ['a']],
      ['Env', ['b', 'c']],
      [null, ['d']],
    ]);
  });
});

describe('nameOf', () => {
  it('prefers aria-label, then the label element, the content, and the title last', () => {
    const root = group(
      '<button aria-label="A" title="T">x</button><button title="A long tooltip">Vue</button>' +
        '<button title="T"></button><button> x  y </button>',
    );
    expect([...root.querySelectorAll('button')].map(nameOf)).toEqual(['A', 'Vue', 'T', 'x y']);
  });
});
