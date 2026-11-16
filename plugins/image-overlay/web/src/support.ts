// The image overlay's page script (`page/image-overlay.js`) against the fake bridge;
// plugins/web-common holds the harness.
import source from '../../page/image-overlay.js?raw';
import { type FakePlugin, type Message, fakePlugin as fake, runPageScriptRoots, settle as rounds } from '../../../web-common/src/harness';

export type { FakePlugin, Message };

export const fakePlugin = (answer: (message: Message) => unknown): FakePlugin => fake('image-overlay', answer);

/** Runs the page script; answers its shadow roots in the order it made them (picture, split). */
export function runPageScript(plugin: FakePlugin): ShadowRoot[] {
  const roots = runPageScriptRoots(source, plugin);
  if (roots.length !== 2) throw new Error(`the page script made ${roots.length} shadow roots`);
  return roots;
}

/** Lets the promises of the fake bridge settle (the overlay decodes its images in steps). */
export const settle = (): Promise<void> => rounds(40);

export function state(overrides: Partial<Record<string, unknown>> = {}): Record<string, unknown> {
  return {
    enabled: true,
    opacity: 50,
    blend: 'difference',
    invert: false,
    anchor: 'document',
    offset: { x: 0, y: 0 },
    split: { on: false, position: 50 },
    autoScaleDpr: true,
    designs: {},
    ...overrides,
  };
}

/** A design's bytes: anything; the fake `createImageBitmap` only checks they arrive whole. */
export function designBytes(length: number): Uint8Array {
  return Uint8Array.from({ length }, (_, index) => (index * 7 + 3) % 256);
}

export function base64(bytes: Uint8Array): string {
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}
