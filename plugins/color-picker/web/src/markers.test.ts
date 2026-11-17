// The calibration markers (page/markers.js) and how the pipette measures a view the window's edge
// or the canvas area cuts (RB-090-40): two markers beside the person's pointer, which is where the
// view is seen; without it, a failed measurement waits for it quietly; a map that starts before
// the image's corner is measured again when the pointer comes back.
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import { type FakePlugin, type Message, fakePlugin, runPageScripts, settle } from './support';
import { calibrateAnswer, tileAnswer } from './tile';

let plugin: FakePlugin;
let shadow: ShadowRoot;
/** What `calibrate` answers; the default maps the viewport at its DPR from the image's corner. */
let measure: (message: Message) => unknown = (message) => calibrateAnswer(message);

function answer(message: Message): unknown {
  if (message.type === 'calibrate') return measure(message);
  if (message.type === 'tile') return tileAnswer(message);
  if (message.type === 'value') return { value: String(message.color) };
  return {};
}

const sent = (type: string): Message[] => plugin.sent.filter((message) => message.type === type);
const layer = (): HTMLElement => shadow.children[1] as HTMLElement;
const pointerAt = (type: string, x: number, y: number) => layer().dispatchEvent(new MouseEvent(type, { clientX: x, clientY: y, bubbles: true }));

/** Lets the page script's promises, its frame waits and the pointer's rest settle. */
async function idle(ms = 300): Promise<void> {
  await settle();
  await new Promise((resolve) => setTimeout(resolve, ms));
  await settle();
}

/** The viewport without its scroll bars, as page/markers.js keeps the markers in it. */
const room = () => ({
  width: Math.min(window.innerWidth, document.documentElement.clientWidth || window.innerWidth),
  height: Math.min(window.innerHeight, document.documentElement.clientHeight || window.innerHeight),
});

beforeEach(() => {
  document.body.innerHTML = '<main><p>Some text</p></main>';
  measure = (message) => calibrateAnswer(message);
  plugin = fakePlugin(answer);
  ({ picker: shadow } = runPageScripts(plugin));
});

afterEach(() => {
  plugin.stop();
});

describe('the markers beside the pointer', () => {
  it('are a pair centred on it, 64 CSS px apart, kept inside the viewport', () => {
    const markers = globalThis.rbColorPickerMarkers;
    expect(markers).toBeDefined();
    expect(markers?.beside({ x: 300, y: 200 })).toEqual([
      { x: 254, y: 192 },
      { x: 318, y: 192 },
    ]);
    expect(markers?.beside({ x: 10, y: 3 })).toEqual([
      { x: 0, y: 0 },
      { x: 64, y: 0 },
    ]);
    const { width, height } = room();
    expect(markers?.beside({ x: width - 2, y: height - 1 })).toEqual([
      { x: width - 92, y: height - 16 },
      { x: width - 28, y: height - 16 },
    ]);
  });

  it('are shown only once the person pointed into the view', async () => {
    plugin.post({ type: 'pick', active: plugin.view, config: { loupeSize: 11 } });
    await idle();
    // The active view at its middle: the corners alone, the pointer not the person's.
    expect(sent('calibrate')[0]).toMatchObject({ pointed: false });
    expect(sent('calibrate')[0]?.marker).toHaveLength(4);
    plugin.post({ type: 'pick', config: { loupeSize: 11 } });
    await idle(50);
    pointerAt('pointermove', 300, 200);
    await idle();
    const [, asked] = sent('calibrate');
    expect(asked).toMatchObject({ pointed: true, x: 300, y: 200 });
    expect(asked?.marker).toHaveLength(6);
    // Hidden again before the read.
    expect((shadow.children[3] as HTMLElement).hidden).toBe(true);
  });
});

describe('a view the window cuts', () => {
  it('measured without the pointer in it, waits for the pointer without a word in the status bar', async () => {
    measure = (message) => (message.pointed === true ? calibrateAnswer(message) : { map: null, reason: 'markers' });
    plugin.post({ type: 'pick', active: plugin.view, config: { loupeSize: 11 } });
    await idle();
    expect(sent('calibrate').length).toBeGreaterThan(0);
    expect(sent('calibrate').every((message) => message.pointed === false)).toBe(true);
    expect(sent('unreadable')).toHaveLength(0);
    expect(sent('tile')).toHaveLength(0);
    pointerAt('pointermove', 300, 200);
    await idle();
    expect(sent('calibrate').at(-1)).toMatchObject({ pointed: true });
    expect(sent('tile')).toHaveLength(1);
    expect(sent('unreadable')).toHaveLength(0);
  });

  it('with the pointer in it and still no map, says so in the status bar', async () => {
    measure = () => ({ map: null, reason: 'markers' });
    plugin.post({ type: 'pick', config: { loupeSize: 11 } });
    await idle(50);
    pointerAt('pointermove', 300, 200);
    await idle(600);
    expect(sent('calibrate')).toHaveLength(3);
    expect(sent('unreadable')[0]).toMatchObject({ reason: 'markers' });
  });

  it('is measured again when the pointer comes back where its map starts before the image', async () => {
    measure = (message) => {
      const dpr = Number(message.dpr) || 1;
      const [width, height] = [Math.round(Number(message.width) * dpr), Math.round(Number(message.height) * dpr)];
      return { map: { left: -40, top: 0, scale: dpr, width, height }, exact: true };
    };
    plugin.post({ type: 'pick', config: { loupeSize: 11 } });
    await idle(50);
    pointerAt('pointermove', 300, 200);
    await idle();
    expect(sent('calibrate')).toHaveLength(1);
    expect(sent('tile')[0]?.map).toMatchObject({ left: -40 });
    pointerAt('pointerleave', 300, 200);
    pointerAt('pointermove', 320, 220);
    await idle();
    expect(sent('calibrate')).toHaveLength(2);
  });

  it('keeps a map from the image corner when the pointer comes back', async () => {
    plugin.post({ type: 'pick', config: { loupeSize: 11 } });
    await idle(50);
    pointerAt('pointermove', 300, 200);
    await idle();
    pointerAt('pointerleave', 300, 200);
    pointerAt('pointermove', 320, 220);
    await idle();
    expect(sent('calibrate')).toHaveLength(1);
  });
});

describe('the loupe in a view the window cuts', () => {
  // diag13, `owner-loupe-cut-right.png`: the loupe right of the pointer, past the canvas area's
  // edge. It goes below right where the view is seen there, flipped left (above) where it is not.
  const LOUPE = { width: 11 * 9 + 8, height: 11 * 9 + 8 + 22 };
  type Area = { left: number; top: number; right: number; bottom: number } | null;
  const place = (at: { x: number; y: number }, seen: unknown, reach: Area = null) => globalThis.rbColorPickerTile?.place(at, LOUPE, 24, seen, reach);
  const loupe = (): HTMLElement => shadow.children[2] as HTMLElement;

  it('goes below right where the viewport has room', () => {
    expect(place({ x: 300, y: 200 }, null)).toMatchObject({ left: 324, top: 224 });
  });

  it('flips left of the pointer where the view is seen only a little further right', () => {
    expect(place({ x: 300, y: 200 }, { left: 0, top: 0, right: 346, bottom: 768 })).toMatchObject({ left: 169, top: 224 });
  });

  it('flips above the pointer where the view is cut below it', () => {
    expect(place({ x: 300, y: 200 }, { left: 0, top: 0, right: 1024, bottom: 260 })).toMatchObject({ left: 324, top: 47 });
  });

  it('counts where the pointer has been as seen', () => {
    const seen = { left: 0, top: 0, right: 346, bottom: 768 };
    expect(place({ x: 300, y: 200 }, seen, { left: 300, top: 200, right: 520, bottom: 200 })).toMatchObject({ left: 324 });
    const reach = globalThis.rbColorPickerTile?.widen(globalThis.rbColorPickerTile?.widen(null, { x: 300, y: 200 }) ?? null, { x: 520, y: 180 });
    expect(reach).toEqual({ left: 300, top: 180, right: 520, bottom: 200 });
  });

  it('with room on neither side takes the side with more', () => {
    expect(place({ x: 300, y: 200 }, { left: 250, top: 0, right: 360, bottom: 768 })).toMatchObject({ left: 324 });
    expect(place({ x: 300, y: 200 }, { left: 230, top: 0, right: 340, bottom: 768 })).toMatchObject({ left: 169 });
  });

  it('takes the viewport for a seen part that is none', () => {
    expect(place({ x: 300, y: 200 }, { left: 'a', top: 0, right: 346, bottom: 768 })).toMatchObject({ left: 324 });
    expect(place({ x: 300, y: 200 }, { left: 0, top: 0, right: 5000, bottom: 9000 })).toMatchObject({ left: 324, top: 224 });
  });

  it('is placed by the part the measurement says is seen', async () => {
    measure = (message) => ({ ...(calibrateAnswer(message) as object), visible: { left: 0, top: 0, right: 346, bottom: 768 } });
    plugin.post({ type: 'pick', config: { loupeSize: 11 } });
    await idle(50);
    pointerAt('pointermove', 300, 200);
    await idle();
    expect(loupe().hidden).toBe(false);
    expect(loupe().style.left).toBe('169px');
    expect(loupe().style.top).toBe('224px');
  });

  it('goes below right without a seen part in the answer', async () => {
    plugin.post({ type: 'pick', config: { loupeSize: 11 } });
    await idle(50);
    pointerAt('pointermove', 300, 200);
    await idle();
    expect(loupe().hidden).toBe(false);
    expect(loupe().style.left).toBe('324px');
  });
});
