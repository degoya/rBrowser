// The rulers and guides' page script (`page/rulers.js`) against the fake bridge;
// plugins/web-common holds the harness.
import source from '../../page/rulers.js?raw';
import { type FakePlugin, type Message, fakePlugin as fake, runPageScript as run } from '../../../web-common/src/harness';

export { settle } from '../../../web-common/src/harness';
export type { FakePlugin, Message };

export const fakePlugin = (answer: (message: Message) => unknown): FakePlugin => fake('rulers-guides', answer);

/** Runs the page script; answers its shadow root (the test opens what the page cannot). */
export const runPageScript = (plugin: FakePlugin): ShadowRoot => run(source, plugin);

export function drawing(overrides: Partial<Record<string, unknown>> = {}): Record<string, unknown> {
  return {
    breakpoint: 'mobile',
    visible: { rulers: true, columns: true, baseline: true },
    guides: [
      { id: 1, axis: 'x', pos: 320 },
      { id: 2, axis: 'y', pos: 200 },
    ],
    columns: {
      left: 0,
      width: 375,
      columns: [
        { x: 16, width: 73.75 },
        { x: 105.75, width: 73.75 },
        { x: 195.5, width: 73.75 },
        { x: 285.25, width: 73.75 },
      ],
    },
    baseline: { lineHeight: 8, offset: 4 },
    ...overrides,
  };
}
