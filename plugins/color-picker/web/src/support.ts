// The color picker's page scripts (`page/marks.js`, `page/tile.js`, then `page/picker.js`, as the manifest runs
// them) against the fake bridge, and its fixture page and collectors in the test's window;
// plugins/web-common holds the harness and the loader.
import marks from '../../page/marks.js?raw';
import picker from '../../page/picker.js?raw';
import tile from '../../page/tile.js?raw';
import { type FakePlugin, type Message, fakePlugin as fake, runPageScriptRoots } from '../../../web-common/src/harness';

export { settle } from '../../../web-common/src/harness';
export { collect, fixturePage, loadPage, pluginFile } from '../../../web-common/src/load';
export type { FakePlugin, Message };

export const fakePlugin = (answer: (message: Message) => unknown): FakePlugin => fake('color-picker', answer);

/** Runs both page scripts; answers their shadow roots (the test opens what the page cannot). */
export function runPageScripts(plugin: FakePlugin): { marks: ShadowRoot; picker: ShadowRoot } {
  const roots = runPageScriptRoots(`${marks}\n${tile}\n${picker}`, plugin);
  const [first, second] = roots;
  if (!first || !second || roots.length !== 2) throw new Error(`the page scripts made ${roots.length} shadow roots, not two`);
  return { marks: first, picker: second };
}

/** Runs both page scripts; answers the pipette's shadow root. */
export const runPageScript = (plugin: FakePlugin): ShadowRoot => runPageScripts(plugin).picker;
