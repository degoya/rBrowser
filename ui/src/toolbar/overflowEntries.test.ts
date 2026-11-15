import { describe, expect, it, vi } from 'vitest';
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

  it('lists the options of a select under its name and chooses through its change event', () => {
    const root = group(
      '<select title="Bookmarks"><option value="" disabled>Bookmarks</option>' +
        '<optgroup label="Shop"><option value="cart">Cart</option></optgroup>' +
        '<option value="home">Home</option><option value="none" disabled>No URL</option></select>',
    );
    const select = root.querySelector('select') as HTMLSelectElement;
    const chosen: string[] = [];
    select.addEventListener('change', () => chosen.push(select.value));
    const entries = overflowEntries([['bookmarks', root]], TEXTS);
    expect(entries.map((entry) => [entry.label, entry.disabled, entry.heading])).toEqual([
      ['Shop / Cart', false, 'Bookmarks'],
      ['Home', false, 'Bookmarks'],
      ['No URL', true, 'Bookmarks'],
    ]);
    entries[1]?.run();
    expect(chosen).toEqual(['home']);
    expect(new Set(entries.map((entry) => entry.key)).size).toBe(3);
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
