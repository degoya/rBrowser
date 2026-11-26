// The color picker's page scripts (`page/marks.js`, `page/tile.js`, `page/markers.js`, then `page/picker.js`, as the manifest runs
// them) against the fake bridge, and its fixture page and collectors in the test's window;
// plugins/web-common holds the harness and the loader.
import markers from '../../page/markers.js?raw';
import marks from '../../page/marks.js?raw';
import picker from '../../page/picker.js?raw';
import tile from '../../page/tile.js?raw';
import { vi } from 'vitest';

import { type FakePlugin, type Message, fakePlugin as fake, runPageScriptRoots, settle } from '../../../web-common/src/harness';

export { settle } from '../../../web-common/src/harness';
export { collect, fixturePage, loadPage, pluginFile } from '../../../web-common/src/load';
export type { FakePlugin, Message };

/**
 * Puts the page script's clocks on Vitest's virtual time (RB-099-18): the pointer's rest
 * (`setTimeout`) and its chained frame waits (`requestAnimationFrame`). `Date` stays real - a
 * test that ages a tile mocks `Date.now` itself.
 */
export function useVirtualTimers(): void {
  vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'requestAnimationFrame', 'cancelAnimationFrame'] });
}

/** Advances the virtual clock by `ms`, the fake bridge's promises settled before and after. */
export async function advance(ms: number): Promise<void> {
  await settle();
  await vi.advanceTimersByTimeAsync(ms);
  await settle();
}

export const fakePlugin = (answer: (message: Message) => unknown): FakePlugin => fake('color-picker', answer);

/** Runs both page scripts; answers their shadow roots (the test opens what the page cannot). */
export function runPageScripts(plugin: FakePlugin): { marks: ShadowRoot; picker: ShadowRoot } {
  const roots = runPageScriptRoots(`${marks}\n${tile}\n${markers}\n${picker}`, plugin);
  const [first, second] = roots;
  if (!first || !second || roots.length !== 2) throw new Error(`the page scripts made ${roots.length} shadow roots, not two`);
  return { marks: first, picker: second };
}

/** Runs both page scripts; answers the pipette's shadow root. */
export const runPageScript = (plugin: FakePlugin): ShadowRoot => runPageScripts(plugin).picker;
