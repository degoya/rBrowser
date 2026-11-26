// The loupe drawn from a tile at fractional device pixel ratios (RB-080-56) and from the view's
// image as it is shown (RB-080-64): the image is the viewport's size at the DPR rounded, so the
// scale the component reports is not the DPR itself; at the image's edge the tile is cut short.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import pixels from '../../src/pixels.rs?raw';
import { advance, type FakePlugin, type Message, fakePlugin, runPageScript, useVirtualTimers } from './support';
import { TILE_HALF, type TileOptions, calibrateAnswer, tileAnswer } from './tile';

let plugin: FakePlugin;
let shadow: ShadowRoot;
let options: TileOptions = {};
const ratio = Object.getOwnPropertyDescriptor(window, 'devicePixelRatio');

/** A CSS column of the document as a colour: `#0000xx`. */
const column = (cssX: number): [number, number, number] => [0, 0, cssX & 0xff];
const hexOf = (cssX: number): string => `#0000${(cssX & 0xff).toString(16).padStart(2, '0')}`;

function answer(message: Message): unknown {
  if (message.type === 'calibrate') return calibrateAnswer(message, options);
  if (message.type === 'tile') return tileAnswer(message, options);
  if (message.type === 'value') return { value: String(message.color) };
  return {};
}

const layer = (): HTMLElement => shadow.children[1] as HTMLElement;
const middleCell = (): string => {
  const cells = (shadow.children[2] as HTMLElement).firstElementChild?.children ?? [];
  return (cells[Math.floor(cells.length / 2)] as HTMLElement | undefined)?.style.background ?? '';
};
/** The tiles the loupe was drawn from. */
const tiles = (): Message[] => plugin.sent.filter((message) => message.type === 'tile');

async function idle(ms = 50): Promise<void> {
  await advance(ms);
}

beforeEach(() => {
  useVirtualTimers();
  document.body.innerHTML = '<main></main>';
  plugin = fakePlugin(answer);
  shadow = runPageScript(plugin);
});

afterEach(() => {
  plugin.stop();
  vi.useRealTimers();
  if (ratio) Object.defineProperty(window, 'devicePixelRatio', ratio);
  options = {};
});

describe('the tile', () => {
  it('takes its half side from src/pixels.rs', () => {
    expect(TILE_HALF).toBeGreaterThan(31);
    expect(pixels).toContain(`pub const TILE_HALF: u32 = ${TILE_HALF};`);
  });

  for (const dpr of [1.25, 1.5]) {
    for (const [name, round, clip] of [['floor', Math.floor, false], ['ceil', Math.ceil, false], ['floor', Math.floor, true], ['ceil', Math.ceil, true]] as const) {
      it(`at DPR ${dpr} (${clip ? 'a clip' : 'the window image'}, ${name}ed) the loupe's middle is the CSS pixel under the pointer`, async () => {
        Object.defineProperty(window, 'devicePixelRatio', { value: dpr, configurable: true });
        options = { round, clip, paint: (cssX) => column(cssX) };
        plugin.post({ type: 'pick', config: { loupeSize: 11, capture: clip ? 'clip' : 'window' } });
        layer().dispatchEvent(new MouseEvent('pointermove', { clientX: 200, clientY: 150, bubbles: true }));
        await idle(300);
        expect(tiles()).toHaveLength(1);
        const reach = Math.floor(TILE_HALF / dpr) - Math.ceil(5 / dpr) - 1;
        for (let x = 200 - reach; x <= 200 + reach; x += 1) {
          layer().dispatchEvent(new MouseEvent('pointermove', { clientX: x, clientY: 150, bubbles: true }));
          expect(middleCell(), `x ${x}`).toContain(hexOf(x));
        }
        await idle(300);
        // Every position inside the tile was drawn from it.
        expect(tiles()).toHaveLength(1);
      });
    }
  }

  for (const dpr of [1, 1.5]) {
    it(`at DPR ${dpr} with the viewport away from the window image's corner the loupe's middle is the CSS pixel under the pointer`, async () => {
      // RB-080-77: the window image shows the viewport from the image pixel (7, 3); the crosshair
      // cell is the measured pixel, the cells around it its neighbours.
      Object.defineProperty(window, 'devicePixelRatio', { value: dpr, configurable: true });
      options = { offset: [7, 3], paint: (cssX) => column(cssX) };
      plugin.post({ type: 'pick', config: { loupeSize: 11 } });
      for (const x of [3, 200, 201, 202]) {
        layer().dispatchEvent(new MouseEvent('pointermove', { clientX: x, clientY: 150, bubbles: true }));
        await idle(300);
        expect(middleCell(), `x ${x}`).toContain(hexOf(x));
      }
      expect(plugin.sent.filter((message) => message.type === 'calibrate')).toHaveLength(1);
    });
  }

  it('at the right and bottom edge of the image a tile cut short still holds the pointer', async () => {
    options = { paint: (cssX) => column(cssX) };
    plugin.post({ type: 'pick', config: { loupeSize: 11 } });
    const [x, y] = [window.innerWidth - 2, window.innerHeight - 1];
    layer().dispatchEvent(new MouseEvent('pointermove', { clientX: x, clientY: y, bubbles: true }));
    await idle(300);
    expect(tiles()).toHaveLength(1);
    expect(middleCell()).toContain(hexOf(x));
    layer().dispatchEvent(new MouseEvent('pointermove', { clientX: x + 1, clientY: y, bubbles: true }));
    expect(middleCell()).toContain(hexOf(x + 1));
    await idle(300);
    expect(tiles()).toHaveLength(1);
  });
});
