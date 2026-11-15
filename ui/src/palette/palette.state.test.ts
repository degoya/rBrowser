// The palette and the shell's state arriving after an open (RB-080-68, review of 2026-11-14): the
// entries are rebuilt once when the state comes, the selection stays on the entry it was on, and
// the state's later pushes cost nothing.
import { describe, expect, it } from 'vitest';
import { shallowRef } from 'vue';
import { sampleState } from '../ipc/mockState';
import type { StateSnapshot } from '../ipc/protocol';
import { createPalette } from './palette';
import { rank } from './rank';
import { createRecent } from './recent';
import { createProviderRegistry } from './registry';
import { createPaletteCore } from './testCore';
import type { PaletteEntry, PaletteProvider } from './types';

const t = (key: string): string => key;

function send(key: string): PaletteEntry['action'] {
  return { kind: 'send', message: { type: 'runCommand', id: key } };
}

/**
 * Three rows without a state, two more ahead of them once there is one - the rows the user's
 * selection was on move down. Counts how often it was asked.
 */
function provider(): PaletteProvider & { calls: number } {
  const counted = {
    id: 'rows',
    calls: 0,
    entries({ state }: { state: StateSnapshot | null }): PaletteEntry[] {
      counted.calls += 1;
      const rows = ['a', 'b', 'c'].map((key) => ({ key, scope: 'command' as const, title: key, action: send(key) }));
      if (state === null) {
        return rows;
      }
      const sized = ['x', 'y'].map((key) => ({ key, scope: 'command' as const, title: key, action: send(key) }));
      return [...sized, ...rows];
    },
  };
  return counted;
}

function setup() {
  const state = shallowRef<StateSnapshot | null>(null);
  const rows = provider();
  const palette = createPalette({
    client: createPaletteCore().client,
    send: () => Promise.resolve(true),
    state: () => state.value,
    t,
    registry: createProviderRegistry([rows]),
    recent: createRecent(null),
  });
  return { palette, state, rows };
}

async function flush(): Promise<void> {
  for (let i = 0; i < 5; i += 1) {
    await Promise.resolve();
  }
}

describe('the palette and a state that comes late', () => {
  it('keeps the selection on its entry when the state rebuilds the rows', async () => {
    const { palette, state } = setup();
    await palette.open();
    expect(palette.results.value.map((entry) => entry.key)).toEqual(['a', 'b', 'c']);
    palette.move(1);
    expect(palette.results.value[palette.active.value]?.key).toBe('b');
    state.value = sampleState();
    await flush();
    expect(palette.results.value.map((entry) => entry.key)).toEqual(['x', 'y', 'a', 'b', 'c']);
    // Enter runs what is highlighted - the entry the user chose, not the one now at its index.
    expect(palette.results.value[palette.active.value]?.key).toBe('b');
  });

  it('starts at the top when the selection was not moved', async () => {
    const { palette, state } = setup();
    await palette.open();
    state.value = sampleState();
    await flush();
    expect(palette.active.value).toBe(0);
  });

  it('rebuilds once when the state comes, not on every later push', async () => {
    const { palette, state, rows } = setup();
    await palette.open();
    const before = rows.calls;
    state.value = sampleState();
    await flush();
    expect(rows.calls).toBe(before + 1);
    for (let i = 0; i < 5; i += 1) {
      state.value = sampleState();
      await flush();
    }
    expect(rows.calls).toBe(before + 1);
  });
});

describe('a pinned entry outside its step', () => {
  it('ranks like any other at the top level', () => {
    const entries: PaletteEntry[] = [
      { key: 'a', scope: 'other', title: 'Alpha', action: send('a') },
      { key: 'custom', scope: 'other', title: 'Custom size', pinned: true, action: send('custom') },
    ];
    expect(rank(entries, '', { recent: [] }).map((entry) => entry.key)).toEqual(['a', 'custom']);
    expect(rank(entries, '', { recent: [], step: null }).map((entry) => entry.key)).toEqual(['a', 'custom']);
  });
});
