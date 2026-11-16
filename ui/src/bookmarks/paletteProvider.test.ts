import { describe, expect, it } from 'vitest';
import { sampleState } from '../ipc/mockState';
import type { ProviderContext } from '../palette/types';
import { bookmarkProvider } from './paletteProvider';

const t = (key: string, args?: Record<string, string | number>): string =>
  args === undefined ? key : `${key} ${JSON.stringify(args)}`;

function context(state: ProviderContext['state'] = sampleState()): ProviderContext {
  return { state, commands: [], settings: null, values: new Map(), t };
}

describe('bookmarkProvider', () => {
  it('lists "Open bookmark: <label>" for every bookmark', () => {
    const entries = bookmarkProvider.entries(context());
    expect(entries.map((e) => e.title)).toEqual([
      'palette.entry.open_bookmark {"label":"Home"}',
      'palette.entry.open_bookmark {"label":"Product"}',
      'palette.entry.open_bookmark {"label":"Checkout"}',
      'palette.entry.open_bookmark {"label":"Storybook"}',
    ]);
    const checkout = entries[2];
    expect(checkout?.key).toBe('bookmark:checkout');
    expect(checkout?.group).toBe('palette.kind.bookmark');
    expect(checkout?.description).toBe('http://localhost:5173/checkout');
    expect(checkout?.keywords).toEqual(['checkout', 'Shop']);
    expect(checkout?.commandId).toBe('bookmarks.open');
    expect(checkout?.action).toEqual({
      kind: 'send',
      message: { type: 'runCommand', id: 'bookmarks.open', args: { id: 'checkout' } },
    });
  });

  it('leaves out a bookmark with nowhere to go and lists nothing without a state', () => {
    const state = sampleState();
    state.bookmarks = [{ id: 'lost', label: 'Lost' }, { id: 'docs', label: 'Docs', url: 'https://d.example', shortcut: 'Ctrl+Shift+F9' }];
    const entries = bookmarkProvider.entries(context(state));
    expect(entries.map((e) => e.key)).toEqual(['bookmark:docs']);
    expect(entries[0]?.shortcut).toBe('Ctrl+Shift+F9');
    expect(bookmarkProvider.entries(context(null))).toEqual([]);
  });
});
