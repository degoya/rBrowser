import { describe, expect, it } from 'vitest';
import { parseQuery, rank } from './rank';
import type { PaletteEntry } from './types';

function entry(key: string, title: string, extra: Partial<PaletteEntry> = {}): PaletteEntry {
  return {
    key,
    scope: 'command',
    title,
    action: { kind: 'send', message: { type: 'runCommand', id: key } },
    ...extra,
  };
}

// Titles of the real registry (locales/en.json), plus the overlay command of RB-060-03.
const COMMANDS: PaletteEntry[] = [
  entry('app.command_palette', 'Command palette', { keywords: ['app.command_palette'] }),
  entry('view.zoom_fit_all', 'Fit every view to its cell', { keywords: ['view.zoom_fit_all'] }),
  entry('devtools.open', 'Open DevTools', { keywords: ['devtools.open'] }),
  entry('view.forward', 'Forward', { keywords: ['view.forward'] }),
  entry('layout.focus_mode', 'Focus mode on or off', { keywords: ['layout.focus_mode'] }),
  entry('overlays.overflow', 'Toggle overflow detector', { keywords: ['overlays.overflow'] }),
  entry('emulation.toggle_reduced_motion', 'Toggle reduced motion', {
    keywords: ['emulation.toggle_reduced_motion'],
  }),
  entry('plugins.list', 'List plugins', { keywords: ['plugins.list'] }),
  entry('view.focus_prev', 'Focus the previous view', { keywords: ['view.focus_prev'] }),
  entry('reload.all', 'Reload all views', { keywords: ['reload.all'] }),
];

describe('parseQuery', () => {
  it('reads the prefixes when they are on', () => {
    expect(parseQuery('>sync', true)).toEqual({ scope: 'command', text: 'sync' });
    expect(parseQuery('@ mobile', true)).toEqual({ scope: 'view', text: 'mobile' });
    expect(parseQuery('#scroll', true)).toEqual({ scope: 'setting', text: 'scroll' });
    expect(parseQuery('  reload ', true)).toEqual({ scope: null, text: 'reload' });
  });

  it('takes a prefix as text when they are off', () => {
    expect(parseQuery('>sync', false)).toEqual({ scope: null, text: '>sync' });
  });
});

describe('rank', () => {
  it('finds "Toggle overflow detector" for "ovfl" under the first three', () => {
    const keys = rank(COMMANDS, 'ovfl').map((e) => e.key);
    expect(keys.slice(0, 3)).toContain('overlays.overflow');
  });

  it('matches every word of the query, in any order', () => {
    const keys = rank(COMMANDS, 'views reload').map((e) => e.key);
    expect(keys[0]).toBe('reload.all');
    expect(rank(COMMANDS, 'reload nothing')).toEqual([]);
  });

  it('searches the keywords and the group', () => {
    expect(rank(COMMANDS, 'zoom_fit').map((e) => e.key)).toEqual(['view.zoom_fit_all']);
    const grouped = [entry('a', 'Alpha', { group: 'Sync' }), entry('b', 'Beta')];
    expect(rank(grouped, 'sync').map((e) => e.key)).toEqual(['a']);
  });

  it('puts the recent entries first without a query, newest first', () => {
    const keys = rank(COMMANDS, '', { recent: ['reload.all', 'devtools.open'] }).map((e) => e.key);
    expect(keys.slice(0, 2)).toEqual(['reload.all', 'devtools.open']);
    expect(keys).toHaveLength(COMMANDS.length);
  });

  it('lifts a recent entry among equal matches', () => {
    const twins = [entry('one', 'Toggle sync'), entry('two', 'Toggle sync')];
    expect(rank(twins, 'sync', { recent: ['two'] }).map((e) => e.key)).toEqual(['two', 'one']);
  });

  it('lists the entries of a step only inside it, and at the top only while typing', () => {
    const entries = [
      entry('step', 'Set view to ...', { action: { kind: 'step', step: 'set-view' } }),
      entry('bp:tablet', 'Set view to Tablet', { parent: 'set-view' }),
    ];
    expect(rank(entries, '').map((e) => e.key)).toEqual(['step']);
    expect(rank(entries, 'tablet').map((e) => e.key)).toEqual(['bp:tablet']);
    expect(rank(entries, '', { step: 'set-view' }).map((e) => e.key)).toEqual(['bp:tablet']);
  });

  it('keeps to the scope of a prefix', () => {
    const entries = [entry('c', 'Mobile thing'), entry('v', 'Mobile', { scope: 'view' })];
    expect(rank(entries, 'mobile', { scope: 'view' }).map((e) => e.key)).toEqual(['v']);
  });

  it('lists a step\'s entries under a bare prefix', () => {
    const entries = [entry('v', 'Mobile', { scope: 'view', parent: 'focus-view' })];
    expect(rank(entries, '', { scope: 'view' }).map((e) => e.key)).toEqual(['v']);
  });
});

// RB-080-68: "Custom size..." stays on top of the new-size picker; a recent device comes after it.
describe('a pinned entry', () => {
  it('leads its step without a query, recent entries after it, and ranks like any other with one', () => {
    const step = { parent: 'new-view' };
    const entries = [
      entry('a', 'Alpha', step),
      entry('b', 'Beta', step),
      entry('custom', 'Custom size', { pinned: true, ...step }),
    ];
    expect(rank(entries, '', { recent: ['b'], step: 'new-view' }).map((e) => e.key)).toEqual(['custom', 'b', 'a']);
    expect(rank(entries, 'beta', { recent: [], step: 'new-view' }).map((e) => e.key)).toEqual(['b']);
  });
});
