// How many screenshots the pipette asks for (RB-080-56): Chromium renders a clipped
// `Page.captureScreenshot` through the device emulation and draws another picture for a frame,
// so a screenshot per pointer move made the views flicker. Red before the fix: 40 screenshots
// for 40 pointer moves, and one in every view at the start and at each synced scroll.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { type FakePlugin, type Message, fakePlugin, pluginFile, runPageScript, settle } from './support';
import { calibrateAnswer, tileAnswer } from './tile';

/** The requests the component answers with a screenshot of the view. */
const CAPTURING = new Set(['sample', 'tile', 'take']);

let plugin: FakePlugin;
let shadow: ShadowRoot;
/** What a `tile` request answers; a test swaps it for a held or failing one. */
let tileReply: (message: Message) => unknown = (message) => tileAnswer(message);
/** What a `calibrate` request answers: the window image's map, or a test's failure. */
let calibrateReply: (message: Message) => unknown = (message) => calibrateAnswer(message);
/** The prefix of a label, standing for the format of the settings. */
let format = 'v';

function answer(message: Message): unknown {
  if (message.type === 'sample') return { value: '#ffffff', grid: Array.from({ length: 121 }, () => '#ffffff') };
  if (message.type === 'calibrate') return calibrateReply(message);
  if (message.type === 'tile') return tileReply(message);
  if (message.type === 'value') return { value: `${format}:${String(message.color)}` };
  return {};
}

/** Holds the next `tile` answers until the test lets them go. */
function holdTiles(): Array<() => void> {
  const held: Array<() => void> = [];
  tileReply = (message) => new Promise((resolve) => held.push(() => resolve(tileAnswer(message))));
  return held;
}

/** The screenshots, without the measurements of the window image (their own tests count them). */
const captures = (): number => plugin.sent.filter((message) => CAPTURING.has(String(message.type))).length;
const sent = (type: string): Message[] => plugin.sent.filter((message) => message.type === type);
const layer = (): HTMLElement => shadow.children[1] as HTMLElement;
const loupe = (): HTMLElement => shadow.children[2] as HTMLElement;
const moveTo = (clientX: number, clientY: number) =>
  layer().dispatchEvent(new MouseEvent('pointermove', { clientX, clientY, bubbles: true }));

async function idle(ms = 50): Promise<void> {
  await settle();
  await new Promise((resolve) => setTimeout(resolve, ms));
  await settle();
}

beforeEach(() => {
  document.body.innerHTML = '<main><p>Some text</p></main>';
  plugin = fakePlugin(answer);
  shadow = runPageScript(plugin);
});

afterEach(() => {
  plugin.stop();
  vi.restoreAllMocks();
  tileReply = (message) => tileAnswer(message);
  calibrateReply = (message) => calibrateAnswer(message);
  format = 'v';
});

describe('the screenshots the pipette takes', () => {
  it('takes none in a view the pointer and the keyboard are not in, not even when it scrolls along', async () => {
    plugin.post({ type: 'pick', config: { loupeSize: 11 } });
    await idle();
    window.dispatchEvent(new Event('scroll'));
    await idle(300);
    expect(captures()).toBe(0);
  });

  it('takes none while the pointer moves, one where it rests', async () => {
    plugin.post({ type: 'pick', config: { loupeSize: 11 } });
    await idle();
    for (let step = 0; step < 40; step += 1) {
      moveTo(10 + step * 20, 30);
      await idle(5);
    }
    expect(captures()).toBe(0);
    await idle(300);
    expect(captures()).toBe(1);
    expect(plugin.sent.find((message) => message.type === 'tile')).toMatchObject({ x: 790, y: 30 });
    expect(loupe().hidden).toBe(false);
  });

  it('draws the loupe from the tile while the pointer stays inside it', async () => {
    plugin.post({ type: 'pick', config: { loupeSize: 11 } });
    moveTo(200, 150);
    await idle(300);
    expect(captures()).toBe(1);
    for (let step = 1; step <= 30; step += 1) {
      moveTo(200 + step, 150 - step);
      await idle(5);
    }
    await idle(300);
    expect(captures()).toBe(1);
    expect(loupe().hidden).toBe(false);
    // The tile's colour at the rest point, the loupe's middle cell when it is back there.
    moveTo(200, 150);
    await idle();
    expect(loupe().textContent).toBe('v:#1e88e5');
    expect(captures()).toBe(1);
  });

  it('takes a fresh tile where the pointer rests next once its tile is two seconds old', async () => {
    plugin.post({ type: 'pick', config: { loupeSize: 11 } });
    moveTo(200, 150);
    await idle(300);
    expect(captures()).toBe(1);
    const taken = Date.now();
    vi.spyOn(Date, 'now').mockReturnValue(taken + 2500);
    moveTo(202, 150);
    // The old tile still draws the moving loupe.
    expect(loupe().hidden).toBe(false);
    await idle(300);
    expect(captures()).toBe(2);
    expect(loupe().hidden).toBe(false);
  });

  it('measures the view once a pipette, by markers in its corners and beside the pointer, and reads by that map', async () => {
    // RB-080-77: the window image showed the markers in the display's colours, and somewhere
    // else than the viewport's size put them; the component finds them anywhere.
    plugin.post({ type: 'pick', config: { loupeSize: 11 } });
    moveTo(100, 80);
    await idle(300);
    const markers = shadow.children[3] as HTMLElement;
    const [measured] = sent('calibrate');
    expect(measured?.marker).toEqual([
      [0, 0],
      [expect.any(Number), expect.any(Number)],
      [expect.any(Number), expect.any(Number)],
      [expect.any(Number), expect.any(Number)],
      [expect.any(Number), expect.any(Number)],
      [expect.any(Number), expect.any(Number)],
    ]);
    // RB-090-19: one in each corner, any two of them enough where the view is shown scaled;
    // RB-090-40: two beside the pointer, where the view is seen when the window cuts it.
    expect(markers.children).toHaveLength(6);
    expect([(markers.children[4] as HTMLElement).style.left, (markers.children[5] as HTMLElement).style.left]).toEqual(['54px', '118px']);
    const corner = (index: number) => markers.children[index] as HTMLElement;
    expect([corner(1).style.right, corner(1).style.top]).toEqual(['0px', '0px']);
    expect([corner(2).style.left, corner(2).style.bottom]).toEqual(['0px', '0px']);
    expect([corner(3).style.right, corner(3).style.bottom]).toEqual(['0px', '0px']);
    // Hidden again; no tile is cut from the image that showed them.
    expect(markers.hidden).toBe(true);
    expect(sent('tile')[0]?.map).toEqual((calibrateAnswer(measured as Message) as { map: unknown }).map);
    expect(sent('tile')[0]?.clip).toBeUndefined();
    expect(loupe().hidden).toBe(false);
    // A scroll keeps the map; no new measurement.
    window.dispatchEvent(new Event('scroll'));
    await idle(300);
    expect(sent('tile')).toHaveLength(2);
    expect(sent('calibrate')).toHaveLength(1);
    // A resize measures again.
    window.dispatchEvent(new Event('resize'));
    await idle(300);
    expect(sent('calibrate')).toHaveLength(2);
  });

  it('asks a window image without markers again, waiting longer, then names the view in the status bar', async () => {
    calibrateReply = () => new Error('plugins.error.invalid');
    const frames = vi.spyOn(window, 'requestAnimationFrame');
    plugin.post({ type: 'pick', config: { loupeSize: 11 } });
    moveTo(100, 80);
    await idle(600);
    expect(sent('calibrate')).toHaveLength(3);
    // 2, 4 and 8 frames before each.
    expect(frames.mock.calls.length).toBeGreaterThanOrEqual(14);
    expect(sent('unreadable')).toEqual([{ type: 'unreadable', reason: 'markers' }]);
    // Never a clip, which flickers; no tile in this pipette.
    expect(captures()).toBe(0);
    moveTo(300, 80);
    await idle(300);
    expect(captures()).toBe(0);
    expect(sent('calibrate')).toHaveLength(3);
  });

  it('hands the component\'s reason to the status bar and does not ask again for an image too large', async () => {
    // RB-090-19, diag11: view 7's image had more than 8 megapixels; the log said
    // `plugins.error.invalid`, the status bar nothing of why.
    calibrateReply = () => ({ map: null, reason: 'size' });
    plugin.post({ type: 'pick', config: { loupeSize: 11 } });
    moveTo(100, 80);
    await idle(600);
    expect(sent('calibrate')).toHaveLength(1);
    expect(sent('unreadable')).toEqual([{ type: 'unreadable', reason: 'size' }]);
    expect(captures()).toBe(0);
  });

  it('asks again where the image did not show the markers, with the last reason', async () => {
    calibrateReply = () => ({ map: null, reason: 'image' });
    plugin.post({ type: 'pick', config: { loupeSize: 11 } });
    moveTo(100, 80);
    await idle(600);
    expect(sent('calibrate')).toHaveLength(3);
    expect(sent('unreadable')).toEqual([{ type: 'unreadable', reason: 'image' }]);
  });

  it('reads clips where the settings ask for them, without markers', async () => {
    plugin.post({ type: 'pick', config: { loupeSize: 11, capture: 'clip' } });
    moveTo(100, 80);
    await idle(300);
    expect(loupe().hidden).toBe(false);
    expect(sent('calibrate')).toHaveLength(0);
    expect(sent('tile')[0]?.clip).toBe(true);
    expect(sent('tile')[0]?.map).toBeUndefined();
    // A new pipette with the window's image measures again.
    plugin.post({ type: 'stop' });
    plugin.post({ type: 'pick', config: { loupeSize: 11 } });
    moveTo(100, 80);
    await idle(300);
    expect(sent('calibrate')).toHaveLength(1);
  });

  it('draws the markers in the colours src/finder.rs looks for', () => {
    const finder = pluginFile('src/finder.rs');
    const markers = pluginFile('page/markers.js');
    expect(finder).toContain('pub const MARKER: [Rgb; 2] = [[0xff, 0x00, 0xff], [0x00, 0xff, 0x00]];');
    expect(finder).toContain('pub const MARKER_SIZE: (f64, f64) = (24.0, 12.0);');
    expect(finder).toContain('pub const MARKER_FRAME: f64 = 2.0;');
    expect(markers).toContain('width:24px;height:12px;border:2px solid #000;');
    expect(markers).toContain('background:#ff00ff;');
    expect(markers).toContain('background:#00ff00;');
  });

  it('takes none after the pointer left the view', async () => {
    plugin.post({ type: 'pick', config: { loupeSize: 11 } });
    moveTo(200, 150);
    layer().dispatchEvent(new MouseEvent('pointerleave'));
    await idle(300);
    expect(captures()).toBe(0);
    expect(loupe().hidden).toBe(true);
  });

  it('takes a new tile after a scroll only in the view the pointer is in', async () => {
    plugin.post({ type: 'pick', config: { loupeSize: 11 } });
    moveTo(200, 150);
    await idle(300);
    window.dispatchEvent(new Event('scroll'));
    window.dispatchEvent(new Event('scroll'));
    await idle(300);
    expect(captures()).toBe(2);
  });

  it('asks the label of a colour once', async () => {
    plugin.post({ type: 'pick', config: { loupeSize: 11 } });
    moveTo(200, 150);
    await idle(300);
    for (let step = 0; step < 10; step += 1) {
      moveTo(200 + (step % 2), 150);
      await idle(5);
    }
    const asked = plugin.sent.filter((message) => message.type === 'value').map((message) => message.color);
    expect(new Set(asked).size).toBe(asked.length);
  });

  it('drops a tile answered after a scroll and takes a new one', async () => {
    const held = holdTiles();
    plugin.post({ type: 'pick', config: { loupeSize: 11 } });
    moveTo(200, 150);
    await idle(300);
    expect(held).toHaveLength(1);
    window.dispatchEvent(new Event('scroll'));
    held[0]!();
    await idle();
    expect(loupe().hidden).toBe(true);
    await idle(300);
    // The map holds after a scroll: one read, no new measurement.
    expect(held).toHaveLength(2);
    held[1]!();
    await idle();
    expect(loupe().hidden).toBe(false);
  });

  it('starts the next pipette without the screenshot of the last one', async () => {
    const held = holdTiles();
    plugin.post({ type: 'pick', config: { loupeSize: 11 } });
    moveTo(200, 150);
    await idle(300);
    plugin.post({ type: 'stop' });
    held[0]!();
    await idle();
    plugin.post({ type: 'pick', config: { loupeSize: 11 } });
    moveTo(201, 150);
    await idle();
    expect(loupe().hidden).toBe(true);
    await idle(300);
    expect(held).toHaveLength(2);
  });

  it('shows no loupe in a view the pointer left while its tile was on the way', async () => {
    const held = holdTiles();
    plugin.post({ type: 'pick', config: { loupeSize: 11 } });
    moveTo(200, 150);
    await idle(300);
    layer().dispatchEvent(new MouseEvent('pointerleave'));
    held[0]!();
    await idle();
    expect(loupe().hidden).toBe(true);
  });

  it('labels in the format of the settings the pipette started with', async () => {
    plugin.post({ type: 'pick', config: { loupeSize: 11 } });
    moveTo(200, 150);
    await idle(300);
    expect(loupe().textContent).toBe('v:#1e88e5');
    plugin.post({ type: 'stop' });
    format = 'w';
    plugin.post({ type: 'pick', config: { loupeSize: 11 } });
    moveTo(200, 150);
    await idle(300);
    expect(loupe().textContent).toBe('w:#1e88e5');
  });

  it('takes a new tile after a resize and after a change of the device pixel ratio', async () => {
    const listeners: Array<() => void> = [];
    vi.spyOn(window, 'matchMedia').mockImplementation(
      (query) =>
        ({
          media: query,
          matches: true,
          addEventListener: (_: string, listener: () => void) => listeners.push(listener),
          removeEventListener: () => undefined,
        }) as unknown as MediaQueryList,
    );
    plugin.post({ type: 'pick', config: { loupeSize: 11 } });
    moveTo(200, 150);
    await idle(300);
    expect(captures()).toBe(1);
    window.dispatchEvent(new Event('resize'));
    expect(loupe().hidden).toBe(true);
    await idle(300);
    expect(captures()).toBe(2);
    expect(listeners.length).toBeGreaterThan(0);
    listeners.at(-1)!();
    expect(loupe().hidden).toBe(true);
    await idle(300);
    expect(captures()).toBe(3);
  });

  it('tries a failed tile once more, then waits for the pointer', async () => {
    tileReply = () => new Error('no tile');
    plugin.post({ type: 'pick', config: { loupeSize: 11 } });
    moveTo(200, 150);
    await idle(300);
    await idle(300);
    await idle(300);
    // The failed read measured the view again before the second.
    const tiles = () => sent('tile').length;
    expect(tiles()).toBe(2);
    expect(sent('calibrate')).toHaveLength(2);
    tileReply = (message) => tileAnswer(message);
    moveTo(260, 150);
    await idle(300);
    expect(tiles()).toBe(3);
    expect(loupe().hidden).toBe(false);
  });

  it('shows the loupe at once in the active view only, one tile at its centre', async () => {
    plugin.post({ type: 'pick', config: { loupeSize: 11 }, active: plugin.view + 1 });
    await idle(300);
    expect(captures()).toBe(0);
    plugin.post({ type: 'pick', config: { loupeSize: 11 }, active: plugin.view });
    await idle(300);
    expect(captures()).toBe(1);
    expect(plugin.sent.find((message) => message.type === 'tile')).toMatchObject({
      x: Math.round(window.innerWidth / 2),
      y: Math.round(window.innerHeight / 2),
    });
    expect(loupe().hidden).toBe(false);
  });
});
