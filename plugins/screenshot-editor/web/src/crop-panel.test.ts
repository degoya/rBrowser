// The screenshot editor panel's crop (RB-080-63) on Nuxt UI (RB-080-86): the frame is a draft
// until Apply or Enter, Apply and Cancel stand at the frame, Undo takes the crop back; and the
// canvas paints once per frame.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { canvas, el, key, keyboardCrop, keyboardRect, lastCall, listed, openEditor, pressed, settle, type Sent } from './mounted';

let sent: Sent[] = [];
let stop: () => void = () => undefined;
const status = (): string => el('#status').textContent ?? '';
const bar = (): HTMLElement => el('#crop-bar');
const click = async (selector: string): Promise<void> => {
  el<HTMLButtonElement>(selector).click();
  await settle();
};
const press = async (target: EventTarget, name: string, init: KeyboardEventInit = {}): Promise<void> => {
  key(target, name, init);
  await settle();
};
const pointer = (type: string, x: number, y: number): void => {
  canvas().dispatchEvent(new PointerEvent(type, { button: 0, clientX: x, clientY: y, bubbles: true, cancelable: true }));
};

beforeEach(async () => {
  sent = [];
  stop = await openEditor(sent);
});

afterEach(() => {
  stop();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe('the crop', () => {
  it('crops with Apply at the frame; the crop is an annotation only after Apply, and Undo takes it back', async () => {
    await keyboardCrop();
    expect(bar().hidden).toBe(false);
    expect(el('#crop-size').textContent).toBe('30 x 20 px');
    expect(el('#crop-apply').getAttribute('aria-keyshortcuts')).toBe('Enter');
    expect(el('#crop-apply').textContent?.trim()).toBe('Apply crop');
    expect(listed()).toEqual([]);
    await press(canvas(), 'Enter');
    expect(listed()).toEqual(['Crop']);
    expect(status()).toBe('Cropped to 30 x 20 px. Ctrl+Z takes it back; the original file stays as it is.');
    expect(bar().hidden).toBe(true);
    expect(pressed()).toBe('rect');
    expect(el('#frame').classList.contains('cropped')).toBe(true);
    await press(document.body, 'z', { ctrlKey: true });
    expect(listed()).toEqual([]);
    expect(el('#frame').classList.contains('cropped')).toBe(false);
  });

  it('resizes the crop draft by a handle with the mouse', async () => {
    await keyboardCrop();
    // The draft runs from (20, 20) to (50, 40); its bottom right handle is dragged 10 px further.
    pointer('pointerdown', 50, 40);
    pointer('pointermove', 55, 45);
    pointer('pointerup', 60, 50);
    await settle();
    expect(el('#crop-size').textContent).toBe('40 x 30 px');
    expect(listed()).toEqual([]);
    await press(canvas(), 'Enter');
    expect(listed()).toEqual(['Crop']);
  });

  it('cancels a crop with Escape or Cancel and keeps the other changes', async () => {
    await keyboardRect();
    await keyboardCrop();
    await press(canvas(), 'Escape');
    expect(bar().hidden).toBe(true);
    expect(listed()).toEqual(['Rectangle']);
    expect(status()).toBe('Crop cancelled.');
    expect(pressed()).toBe('rect');
    await keyboardCrop();
    await click('#crop-cancel');
    expect(listed()).toEqual(['Rectangle']);
    await keyboardCrop();
    await click('#crop-apply');
    expect(listed()).toEqual(['Rectangle', 'Crop']);
    await press(canvas(), 'c');
    expect(bar().hidden).toBe(false);
    await click('#crop-remove');
    expect(listed()).toEqual(['Rectangle']);
    expect(status()).toBe('Crop removed.');
  });

  // Review of RB-080-63, 1: Undo in the crop tool left the old frame as the draft, and Enter
  // applied the undone crop again.
  it('follows Undo with the crop draft', async () => {
    await keyboardCrop();
    await press(canvas(), 'Enter');
    await press(canvas(), 'c');
    expect(bar().hidden).toBe(false);
    await press(document.body, 'z', { ctrlKey: true });
    expect(listed()).toEqual([]);
    expect(bar().hidden).toBe(true);
    await press(canvas(), 'Enter');
    expect(listed()).toEqual([]);
  });

  // Review of RB-080-63, 2: applying the unchanged crop again added an undo step and made the
  // image dirty; Enter on it could not start a new frame.
  it('applies an unchanged crop as nothing, and Enter on it starts a new frame', async () => {
    await keyboardCrop();
    await press(canvas(), 'Enter');
    await press(canvas(), 'c');
    await click('#crop-apply');
    expect(listed()).toEqual(['Crop']);
    await press(document.body, 'z', { ctrlKey: true });
    expect(listed()).toEqual([]);
    await press(document.body, 'y', { ctrlKey: true });
    await press(canvas(), 'c');
    await press(canvas(), 'Enter');
    expect(status()).toBe('Move to the second point and press Enter.');
  });

  // Review of RB-080-63, 4: a cancelled pointer left the grab, and the next moves kept resizing.
  it('drops a grab when the pointer is cancelled', async () => {
    await keyboardCrop();
    pointer('pointerdown', 50, 40);
    pointer('pointercancel', 50, 40);
    pointer('pointermove', 70, 60);
    await settle();
    expect(el('#crop-size').textContent).toBe('30 x 20 px');
  });

  // Review of RB-080-63, 5: Escape in the crop tool without a draft said "Crop cancelled." and
  // discarded nothing; choosing Crop again dropped an unapplied draft.
  it('keeps a draft when Crop is chosen again; Escape without one discards as its tooltip says', async () => {
    await keyboardCrop();
    await press(canvas(), 'c');
    expect(el('#crop-size').textContent).toBe('30 x 20 px');
    await press(canvas(), 'Escape');
    expect(status()).toBe('Crop cancelled.');
    await keyboardRect();
    await press(canvas(), 'c');
    await press(canvas(), 'Escape');
    expect(listed()).toEqual([]);
    expect(status()).toBe('Changes discarded.');
  });

  // Review of RB-080-63, 3: the keyboard cursor started in the middle of the whole image, maybe
  // outside the cropped view.
  it('starts the keyboard cursor in the middle of the cropped view', async () => {
    await keyboardCrop();
    await press(canvas(), 'Enter');
    await press(canvas(), 'Enter');
    await press(canvas(), 'ArrowRight', { shiftKey: true });
    await press(canvas(), 'ArrowDown', { shiftKey: true });
    await press(canvas(), 'Enter');
    key(document.body, 's', { ctrlKey: true });
    expect(lastCall(sent, 'annotate')?.args?.annotations).toContainEqual(expect.objectContaining({ type: 'rect', x: 35, y: 30, w: 10, h: 10 }));
  });

  // Review of RB-080-63, 8: every change painted at once, several times per action.
  it('paints the canvas once per frame', async () => {
    // The paint the load asked for runs first.
    await new Promise((resolve) => requestAnimationFrame(() => resolve(undefined)));
    const frames: FrameRequestCallback[] = [];
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => frames.push(callback));
    const painted = vi.spyOn(HTMLCanvasElement.prototype, 'getContext');
    const own = () => painted.mock.contexts.filter((target) => target === canvas()).length;
    painted.mockClear();
    await keyboardCrop();
    await press(canvas(), 'Enter');
    expect(own()).toBe(0);
    expect(frames).toHaveLength(1);
    frames.splice(0).forEach((callback) => callback(0));
    expect(own()).toBe(1);
  });

  it('keeps the zoom the person chose when a crop is applied', async () => {
    await press(document.body, '+');
    const zoomed = el('#zoom-value').textContent;
    await keyboardCrop();
    await press(canvas(), 'Enter');
    expect(el('#zoom-value').textContent).toBe(zoomed);
  });
});
