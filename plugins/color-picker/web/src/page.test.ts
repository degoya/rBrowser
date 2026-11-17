// The page scripts (page/picker.js, page/marks.js) against a fake bridge: the pipette by mouse and
// keyboard, the element choice, the marks, and that everything lives in `rbrowser-color-picker`
// and `rbrowser-color-picker-marks`.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { type FakePlugin, type Message, fakePlugin, runPageScripts, settle } from './support';
import { calibrateAnswer, tileAnswer } from './tile';

const GRID = Array.from({ length: 121 }, (_, index) => (index === 60 ? '#1e88e5' : '#ffffff'));

let plugin: FakePlugin;
let shadow: ShadowRoot;
let marksShadow: ShadowRoot;

function answer(message: Message): unknown {
  if (message.type === 'sample') return { value: '#1e88e5', color: { hex: '#1e88e5' }, grid: GRID };
  if (message.type === 'calibrate') return calibrateAnswer(message);
  if (message.type === 'tile') return tileAnswer(message);
  if (message.type === 'value') return { value: String(message.color) };
  if (message.type === 'take') return { value: '#1e88e5' };
  return {};
}

/** The pipette's shadow root's direct children: the choice's box, layer, loupe; and the marks. */
function parts() {
  const [, layer, loupe] = [...shadow.children] as HTMLElement[];
  return { marks: marksShadow.firstElementChild as HTMLElement, layer: layer!, loupe: loupe! };
}

function sent(type: string): Message[] {
  return plugin.sent.filter((message) => message.type === type);
}

/** Lets the page script's promises, its two-frame waits and the pointer's rest settle. */
async function idle(ms = 50): Promise<void> {
  await settle();
  await new Promise((resolve) => setTimeout(resolve, ms));
  await settle();
}

function key(name: string, shiftKey = false): KeyboardEvent {
  const event = new KeyboardEvent('keydown', { key: name, shiftKey, bubbles: true, cancelable: true });
  document.body.dispatchEvent(event);
  return event;
}

beforeEach(() => {
  document.body.innerHTML = '<main><p id="text">Some text</p><p>More</p></main>';
  plugin = fakePlugin(answer);
  ({ picker: shadow, marks: marksShadow } = runPageScripts(plugin));
});

afterEach(() => {
  plugin.stop();
  vi.restoreAllMocks();
});

describe('the pipette', () => {
  it('lives in rbrowser-color-picker, so the sync agent ignores it', async () => {
    plugin.post({ type: 'pick', config: { loupeSize: 11 } });
    await idle();
    const host = document.querySelector('rbrowser-color-picker');
    expect(host).not.toBeNull();
    expect(host?.shadowRoot).toBeNull();
    expect(parts().layer.hidden).toBe(false);
  });

  it('takes a tile where the pointer rests and draws the loupe from it', async () => {
    plugin.post({ type: 'pick', config: { loupeSize: 11 } });
    await idle();
    expect(sent('tile')).toHaveLength(0);
    parts().layer.dispatchEvent(new MouseEvent('pointermove', { clientX: 300, clientY: 200, bubbles: true }));
    await idle(300);
    const [first] = sent('tile');
    // A viewport point with the viewport's size: the component maps it into the view's image.
    expect(first).toMatchObject({ x: 300, y: 200, width: window.innerWidth, height: window.innerHeight, scrollX: 0, scrollY: 0 });
    // Read by the map the pipette measured first, with its four markers and two beside the
    // pointer (RB-080-77, RB-090-19, RB-090-40).
    expect(sent('calibrate')[0]?.marker).toHaveLength(6);
    expect(sent('calibrate')[0]?.pointed).toBe(true);
    expect(first?.map).toMatchObject({ left: 0, top: 0 });
    expect(first?.marker).toBeUndefined();
    expect(first?.dpr).toBe(window.devicePixelRatio || 1);
    const { loupe } = parts();
    expect(loupe.hidden).toBe(false);
    expect(loupe.textContent).toBe('#1e88e5');
    const cells = loupe.firstElementChild?.children ?? [];
    expect(cells).toHaveLength(121);
    expect((cells[60] as HTMLElement).style.background).toContain('#1e88e5');
  });

  it('takes the colour by click and the background by Shift+click', async () => {
    plugin.post({ type: 'pick', config: { loupeSize: 11 } });
    await idle();
    parts().layer.dispatchEvent(new MouseEvent('click', { clientX: 40, clientY: 60, bubbles: true }));
    await idle();
    expect(sent('take')[0]).toMatchObject({ slot: 'foreground', x: 40, y: 60 });
    expect(parts().layer.hidden).toBe(true);
    plugin.post({ type: 'pick', config: { loupeSize: 11 } });
    await idle();
    parts().layer.dispatchEvent(new MouseEvent('click', { clientX: 5, clientY: 6, shiftKey: true, bubbles: true }));
    await idle();
    expect(sent('take')[1]).toMatchObject({ slot: 'background', x: 5, y: 6 });
  });

  it('moves by arrow keys, takes by Enter and ends by Esc - keys the page never sees', async () => {
    const seen = vi.fn();
    document.addEventListener('keydown', seen);
    plugin.post({ type: 'pick', config: { loupeSize: 11 } });
    await idle();
    const start = { x: Math.round(window.innerWidth / 2), y: Math.round(window.innerHeight / 2) };
    expect(key('ArrowRight').defaultPrevented).toBe(true);
    await idle(300);
    expect(sent('tile')[0]).toMatchObject({ x: start.x + 1, y: start.y });
    expect(parts().loupe.hidden).toBe(false);
    key('ArrowDown', true);
    await idle(300);
    // Ten pixels down stay inside the tile: no screenshot after the measurement and the read.
    expect(sent('calibrate')).toHaveLength(1);
    expect(sent('tile')).toHaveLength(1);
    key('Enter', true);
    await idle();
    expect(sent('take')[0]).toMatchObject({ slot: 'background', x: start.x + 1, y: start.y + 10 });
    plugin.post({ type: 'pick', config: { loupeSize: 11 } });
    await idle();
    key('Escape');
    expect(parts().layer.hidden).toBe(true);
    expect(seen).not.toHaveBeenCalled();
    // Esc ends the pipette in every view and tells the panel, through the component.
    expect(sent('stopped')).toHaveLength(1);
    // Idle again: keys reach the page.
    key('ArrowLeft');
    expect(seen).toHaveBeenCalledTimes(1);
  });

  it('takes the focus while picking and gives it back', async () => {
    document.body.insertAdjacentHTML('beforeend', '<input id="field">');
    const field = document.getElementById('field') as HTMLInputElement;
    field.focus();
    plugin.post({ type: 'pick', config: { loupeSize: 11 } });
    await idle();
    expect(document.activeElement?.localName).toBe('rbrowser-color-picker');
    key('Escape');
    expect(document.activeElement).toBe(field);
  });

  // RB-090-64: a focused layer under an aria-hidden ancestor made Chrome warn in the page's console.
  it('takes the focus as an interactive layer with a role and its name, never under aria-hidden', async () => {
    plugin.post({ type: 'pick', config: { loupeSize: 11 }, label: 'Colour picker' });
    await idle();
    const host = document.activeElement as HTMLElement;
    expect(host.localName).toBe('rbrowser-color-picker');
    expect(host.closest('[aria-hidden="true"]')).toBeNull();
    expect(host.getAttribute('role')).toBe('dialog');
    expect(host.getAttribute('aria-label')).toBe('Colour picker');
  });

  it('names its layer when choosing too', async () => {
    plugin.post({ type: 'choose', label: 'Colour picker' });
    await idle();
    const host = document.querySelector('rbrowser-color-picker');
    expect(host?.hasAttribute('aria-hidden')).toBe(false);
    expect(host?.getAttribute('aria-label')).toBe('Colour picker');
  });

  it('marks stay out of the accessibility tree', () => {
    plugin.post({ type: 'lines', marks: [{ x: 1, y: 1, width: 1, height: 1, chars: 81 }] });
    expect(document.querySelector('rbrowser-color-picker-marks')?.getAttribute('aria-hidden')).toBe('true');
  });

  it('ends by Esc while choosing without the stop of a pipette', async () => {
    plugin.post({ type: 'choose' });
    await idle();
    key('Escape');
    expect(parts().layer.hidden).toBe(true);
    expect(sent('stopped')).toHaveLength(0);
  });

  it('hides the marks while reading pixels and stops when the component says so', async () => {
    plugin.post({ type: 'lines', marks: [{ x: 1, y: 2, width: 300, height: 20, chars: 92 }] });
    plugin.post({ type: 'pick', config: { loupeSize: 5 } });
    parts().layer.dispatchEvent(new MouseEvent('pointermove', { clientX: 100, clientY: 100, bubbles: true }));
    await idle(300);
    expect(parts().marks.style.visibility).toBe('hidden');
    expect(parts().loupe.firstElementChild?.children).toHaveLength(25);
    plugin.post({ type: 'stop' });
    expect(parts().layer.hidden).toBe(true);
    expect(parts().marks.style.visibility).toBe('visible');
  });
});

describe('choosing the element to measure', () => {
  it('sends a selector that names the clicked element', async () => {
    const target = document.querySelectorAll('p')[1]!;
    document.elementsFromPoint = () => [document.querySelector('rbrowser-color-picker')!, target, document.body];
    plugin.post({ type: 'choose' });
    await idle();
    parts().layer.dispatchEvent(new MouseEvent('click', { clientX: 10, clientY: 10, bubbles: true }));
    await idle();
    const selector = String(sent('chosen')[0]?.selector);
    expect(document.querySelector(selector)).toBe(target);
    expect(parts().layer.hidden).toBe(true);
  });

  it('prefers a unique id', async () => {
    const target = document.getElementById('text')!;
    document.elementsFromPoint = () => [target];
    plugin.post({ type: 'choose' });
    await idle();
    key('ArrowRight');
    key('Enter');
    await idle();
    expect(sent('chosen')[0]?.selector).toBe('#text');
  });
});

describe('marks', () => {
  it('draws the long lines with their characters and clears them', () => {
    plugin.post({ type: 'lines', marks: [{ x: 24, y: 100, width: 900, height: 27, chars: 151 }] });
    const box = parts().marks.firstElementChild?.firstElementChild as HTMLElement;
    expect(box.style.left).toBe('24px');
    expect(box.style.width).toBe('900px');
    expect(box.textContent).toBe('151');
    plugin.post({ type: 'clear' });
    expect(parts().marks.firstElementChild?.childElementCount).toBe(0);
  });

  it('outlines the elements using a palette colour', () => {
    document.body.innerHTML = '<div style="background-color: rgb(30, 136, 229)">a</div><div>b</div>';
    vi.spyOn(Element.prototype, 'getClientRects').mockImplementation(() => [new DOMRect(0, 0, 10, 10)] as unknown as DOMRectList);
    plugin.post({ type: 'mark', values: ['rgb(30, 136, 229)'] });
    const usesBox = parts().marks.children[1]!;
    expect(usesBox.childElementCount).toBe(1);
    plugin.post({ type: 'unmark' });
    expect(usesBox.childElementCount).toBe(0);
  });

  it('live in rbrowser-color-picker-marks and never take the pointer', () => {
    plugin.post({ type: 'lines', marks: [{ x: 1, y: 1, width: 1, height: 1, chars: 81 }] });
    const host = document.querySelector('rbrowser-color-picker-marks') as HTMLElement | null;
    expect(host?.shadowRoot).toBeNull();
    expect(host?.style.pointerEvents).toBe('none');
  });

  it('show again when choosing starts after a pipette', () => {
    plugin.post({ type: 'pick', config: { loupeSize: 5 } });
    expect(parts().marks.style.visibility).toBe('hidden');
    plugin.post({ type: 'choose' });
    expect(parts().marks.style.visibility).toBe('visible');
  });

  it('removes everything when the host stops the script', async () => {
    plugin.post({ type: 'lines', marks: [{ x: 1, y: 1, width: 1, height: 1, chars: 81 }] });
    plugin.post({ type: 'pick', config: { loupeSize: 11 } });
    await idle();
    expect(document.querySelector('rbrowser-color-picker')).not.toBeNull();
    expect(document.querySelector('rbrowser-color-picker-marks')).not.toBeNull();
    // RB-090-51: both in rBrowser's one host of page overlays, which leaves with them.
    expect(document.querySelectorAll('rbrowser-layers > [data-rbrowser-plugin="color-picker"]')).toHaveLength(2);
    plugin.stop();
    expect(document.querySelector('rbrowser-color-picker')).toBeNull();
    expect(document.querySelector('rbrowser-color-picker-marks')).toBeNull();
    expect(document.querySelector('rbrowser-layers')).toBeNull();
  });
});
