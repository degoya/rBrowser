// The screenshot editor panel on Nuxt UI (RB-080-86): the calls it sends, the canvas drawn by
// keyboard, the tools and their options, saving, copying and opening. The crop is
// crop-panel.test.ts.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  answer,
  callsOf,
  canvas,
  el,
  fromHost,
  key,
  keyboardRect,
  lastCall,
  listed,
  openEditor,
  settle,
  SHOT,
  FIRST,
  visible,
  type Sent,
} from './mounted';

let sent: Sent[] = [];
let stop: () => void = () => undefined;
const status = (): string => el('#status').textContent ?? '';
const tone = (): string | undefined => el('#status').dataset.tone;
const click = async (selector: string): Promise<void> => {
  el<HTMLButtonElement>(selector).click();
  await settle();
};

/** The tooltip `target` opens on focus: its text and keys. */
async function tooltip(target: HTMLElement): Promise<string> {
  target.dispatchEvent(new FocusEvent('focus'));
  target.focus();
  await settle();
  const shown = [...document.querySelectorAll('[role="tooltip"]')].map((node) => (node.textContent ?? '').replace(/\s+/g, ' ').trim());
  target.blur();
  await settle();
  return shown.join(' | ');
}

/** The file select's groups and items, opened by its trigger. */
async function fileMenu(): Promise<string[]> {
  const trigger = el('#file');
  trigger.dispatchEvent(new PointerEvent('pointerdown', { button: 0, pointerType: 'mouse', bubbles: true }));
  trigger.click();
  await settle();
  return [...document.querySelectorAll('[role="listbox"] [data-slot="label"], [role="listbox"] [role="option"]')].map((node) =>
    `${node.getAttribute('role') === 'option' ? '' : '# '}${(node.textContent ?? '').trim()}`,
  );
}

beforeEach(async () => {
  sent = [];
  stop = await openEditor(sent);
});

afterEach(() => {
  stop();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe('the screenshot editor panel', () => {
  it('asks for its texts, lists the images and loads the newest screenshot', () => {
    expect(sent[0]).toEqual({ type: 'settings' });
    expect(lastCall(sent, 'list')?.args).toEqual({});
    expect(callsOf(sent).find((call) => call.tool === 'load')?.args).toEqual({});
    expect(el('#title').textContent).toBe('Screenshot editor');
    expect(el('#file').textContent?.trim()).toBe('screenshots/2026-11-12_10-00-00/mobile.png');
    expect([canvas().width, canvas().height]).toEqual([100, 80]);
    expect(status()).toBe(`${SHOT}: 100 x 80 px.`);
    expect(el('#color').dataset.value).toBe('#00ff00');
    expect(el('#stroke [role="slider"]').getAttribute('aria-valuenow')).toBe('6');
  });

  it('offers the images grouped by folder', async () => {
    expect(await fileMenu()).toEqual(['# screenshots/2026-11-12_10-00-00', 'mobile.png', '# checks/run', 'diff.png']);
  });

  it('labels every control and names each tool with its key', async () => {
    for (const control of document.querySelectorAll('button, input, [role="slider"]')) {
      const by = control.getAttribute('aria-labelledby');
      const name = control.getAttribute('aria-label') ?? (by ? document.getElementById(by)?.textContent : null) ?? control.textContent;
      expect(name?.trim(), control.outerHTML.slice(0, 80)).toBeTruthy();
    }
    const tools = [...document.querySelectorAll<HTMLElement>('[data-tool]')];
    expect(tools).toHaveLength(9);
    const redact = el('[data-tool="redact"]');
    expect(redact.getAttribute('aria-label')).toBe('Redact');
    expect(redact.getAttribute('aria-keyshortcuts')).toBe('b');
    expect(await tooltip(redact)).toBe('Redact B');
    expect(el('#tools').getAttribute('role')).toBe('toolbar');
    expect(el('#tools').getAttribute('aria-label')).toBe('Tools');
    expect(canvas().getAttribute('aria-label')).toBe(`Screenshot ${SHOT} with 0 annotations`);
  });

  it('chooses a tool by its key and shows it pressed', async () => {
    key(canvas(), 'a');
    await settle();
    expect(el('[data-tool="arrow"]').getAttribute('aria-pressed')).toBe('true');
    expect(el('[data-tool="rect"]').getAttribute('aria-pressed')).toBe('false');
    expect(status()).toBe('Tool: Arrow.');
    // Not while typing into a field.
    key(canvas(), 't');
    key(canvas(), 'Enter');
    await settle();
    key(el('#text-input'), 'c');
    await settle();
    expect(el('[data-tool="crop"]').getAttribute('aria-pressed')).toBe('false');
  });

  it('draws by keyboard alone and undoes and redoes it', async () => {
    await keyboardRect();
    expect(listed()).toEqual(['Rectangle']);
    expect(status()).toBe('Rectangle added.');
    expect(canvas().getAttribute('aria-label')).toBe(`Screenshot ${SHOT} with 1 annotation`);
    key(document.body, 'z', { ctrlKey: true });
    await settle();
    expect(listed()).toEqual([]);
    key(document.body, 'y', { ctrlKey: true });
    await settle();
    expect(listed()).toEqual(['Rectangle']);
    await click('#undo');
    expect(listed()).toEqual([]);
    expect(el<HTMLButtonElement>('#redo').disabled).toBe(false);
  });

  it('cancels a drawing in progress with Escape', async () => {
    canvas().focus();
    key(canvas(), 'Enter');
    await settle();
    expect(status()).toBe('Move to the second point and press Enter.');
    key(canvas(), 'Escape');
    key(canvas(), 'ArrowRight', { shiftKey: true });
    key(canvas(), 'Enter');
    key(canvas(), 'ArrowDown', { shiftKey: true });
    key(canvas(), 'ArrowRight', { shiftKey: true });
    key(canvas(), 'Enter');
    await settle();
    expect(listed()).toEqual(['Rectangle']);
  });

  it('adds text through its field: Enter confirms, Escape cancels', async () => {
    canvas().focus();
    key(canvas(), 't');
    key(canvas(), 'Enter');
    await settle();
    const field = el<HTMLInputElement>('#text-input');
    expect(document.activeElement).toBe(field);
    field.value = 'Login fails';
    field.dispatchEvent(new Event('input'));
    key(field, 'Enter');
    await settle();
    expect(document.querySelector('#text-entry')).toBeNull();
    expect(listed()).toEqual(['Text: Login fails']);
    key(canvas(), 'Enter');
    await settle();
    key(el('#text-input'), 'Escape');
    await settle();
    expect(document.querySelector('#text-entry')).toBeNull();
    expect(listed()).toHaveLength(1);
  });

  it('saves through shot_annotate and discards unsaved changes with Escape', async () => {
    await keyboardRect();
    key(document.body, 's', { ctrlKey: true });
    const save = lastCall(sent, 'annotate');
    expect(save?.args?.image).toBe(SHOT);
    expect(save?.args?.annotations).toEqual([{ type: 'rect', x: 50, y: 40, w: 30, h: 20, color: '#00ff00', width: 6, fill: false }]);
    await answer(sent, 'annotate', { path: '.rbrowser/out/screenshot-editor/screenshots/2026-11-12_10-00-00/mobile.edited.png' });
    expect(status()).toBe('Saved to .rbrowser/out/screenshot-editor/screenshots/2026-11-12_10-00-00/mobile.edited.png.');
    expect(tone()).toBe('ok');
    key(canvas(), 'm');
    key(canvas(), 'Enter');
    await settle();
    expect(listed()).toEqual(['Rectangle', 'Number']);
    key(document.body, 'Escape');
    await settle();
    expect(listed()).toEqual(['Rectangle']);
    expect(status()).toBe('Changes discarded.');
  });

  it('removes one annotation from the list', async () => {
    await keyboardRect();
    key(canvas(), 'm');
    key(canvas(), 'Enter');
    await settle();
    const remove = el<HTMLButtonElement>('#annotations li [data-remove]');
    expect(remove.getAttribute('aria-label')).toBe('Remove annotation 1: Rectangle');
    remove.click();
    await settle();
    expect(listed()).toEqual(['Number']);
  });

  it('shows a failed save with the error text', async () => {
    await keyboardRect();
    await click('#save');
    await answer(sent, 'annotate', 'plugin.screenshot-editor.errors.path_outside', false);
    expect(status()).toContain('opens PNG files below .rbrowser/out/ only');
    expect(tone()).toBe('error');
  });

  it('loads an image in chunks and its saved annotations', async () => {
    fromHost({ type: 'event', message: { open: '.rbrowser/out/checks/run/diff.png' } });
    await settle();
    expect(lastCall(sent, 'load')?.args).toEqual({ image: '.rbrowser/out/checks/run/diff.png' });
    await answer(sent, 'load', { ...FIRST, image: '.rbrowser/out/checks/run/diff.png', next: 3, annotations: [{ type: 'marker', x: 1, y: 1, number: 1, color: '#000000', size: 12 }] });
    expect(lastCall(sent, 'load')?.args).toEqual({ image: '.rbrowser/out/checks/run/diff.png', offset: 3, hash: 'h1' });
    await answer(sent, 'load', { image: '.rbrowser/out/checks/run/diff.png', offset: 3, next: null, hash: 'h1', data: btoa('!!') });
    expect(listed()).toEqual(['Number']);
    expect(el('#file').textContent?.trim()).toBe('checks/run/diff.png');
  });

  it('asks before an open discards unsaved changes', async () => {
    await keyboardRect();
    await fileMenu();
    const diff = [...document.querySelectorAll<HTMLElement>('[role="option"]')].find((option) => option.textContent?.includes('diff.png'));
    // Chosen by keyboard, as reka-ui's select takes a key on its item.
    diff?.focus();
    if (diff) key(diff, 'Enter');
    await settle();
    expect(el('#file').textContent?.trim()).toBe('checks/run/diff.png');
    const before = callsOf(sent).length;
    await click('#open');
    expect(callsOf(sent).length).toBe(before);
    expect(status()).toBe('There are unsaved changes. Press Open again to discard them.');
    await click('#open');
    expect(lastCall(sent, 'load')?.args).toEqual({ image: '.rbrowser/out/checks/run/diff.png' });
  });

  it('copies the edited image to the clipboard when the command asks', async () => {
    const write = vi.fn(async () => undefined);
    vi.stubGlobal('navigator', { clipboard: { write } });
    vi.stubGlobal(
      'ClipboardItem',
      class {
        constructor(readonly items: Record<string, Blob>) {}
      },
    );
    vi.spyOn(HTMLCanvasElement.prototype, 'toBlob').mockImplementation(function (callback: BlobCallback) {
      callback(new Blob(['png'], { type: 'image/png' }));
    });
    fromHost({ type: 'event', message: { copy: true } });
    await settle();
    expect(write).toHaveBeenCalledTimes(1);
    expect(status()).toBe('The edited image is on the clipboard.');
    write.mockRejectedValueOnce(new Error('NotAllowedError'));
    await click('#copy');
    expect(status()).toBe('The clipboard did not take the image (NotAllowedError). Save it and copy the file instead.');
  });

  it('zooms with + and - and fits with 0', async () => {
    key(document.body, '+');
    await settle();
    expect(el('#zoom-value').textContent).toBe('150 %');
    const zoomed = canvas().style.width;
    key(document.body, '-');
    await settle();
    expect(canvas().style.width).not.toBe(zoomed);
    key(document.body, '0');
    await settle();
    expect(el('#zoom-value').textContent).toBe('100 %');
  });

  // Review of RB-080-59: the window keeps the editor's frame alive while it holds unsaved work.
  it('tells the window while it holds unsaved work', async () => {
    const dirt = () => sent.filter((message) => message.type === 'dirty').map((message) => message.dirty);
    expect(dirt()).toEqual([]);
    await keyboardRect();
    expect(dirt()).toEqual([true]);
    await click('#save');
    await answer(sent, 'annotate', { path: '.rbrowser/out/screenshot-editor/mobile.edited.png' });
    expect(dirt()).toEqual([true, false]);
  });

  // RB-080-63: the owner found a tool's options far from the tool.
  it('shows the chosen tool and only its options beside the tools', async () => {
    expect(el('#context-name').textContent).toBe('Rectangle');
    expect(el('#context').getAttribute('aria-label')).toBe('Rectangle options');
    expect(visible()).toEqual(['color', 'width', 'fill']);
    key(canvas(), 't');
    await settle();
    expect(visible()).toEqual(['color', 'size']);
    key(canvas(), 'b');
    await settle();
    expect(el('#context-name').textContent).toBe('Redact');
    expect(visible()).toEqual(['mode', 'block']);
    const black = [...document.querySelectorAll<HTMLElement>('#mode [role="radio"]')].find((radio) => radio.closest('[data-slot="item"]')?.textContent?.includes('Black bar'));
    black?.click();
    await settle();
    expect(visible()).toEqual(['mode']);
    key(canvas(), 'c');
    await settle();
    expect(visible()).toEqual(['crop']);
  });

  it('shows the tools as icons and names every shortcut in its tooltip', async () => {
    const rect = el('[data-tool="rect"]');
    expect(rect.querySelector('[data-slot="leadingIcon"]')).not.toBeNull();
    expect(rect.getAttribute('aria-label')).toBe('Rectangle');
    expect(rect.textContent?.trim()).toBe('');
    expect(await tooltip(rect)).toBe('Rectangle R');
    expect(await tooltip(el('#undo'))).toBe('Undo Ctrl Z');
    expect(await tooltip(el('#redo'))).toBe('Redo Ctrl Y');
    expect(await tooltip(el('#save'))).toBe('Save Ctrl S');
    expect(await tooltip(el('#zoom-fit'))).toBe('Fit 0');
    expect(el('#save').textContent?.trim()).toBe('Save');
    expect(el('#save').hasAttribute('data-primary')).toBe(true);
    expect(el('#undo').getAttribute('aria-keyshortcuts')).toBe('Control+Z');
    for (const id of ['undo', 'redo', 'revert', 'copy', 'zoom-in', 'zoom-out', 'zoom-fit', 'refresh']) {
      const button = el(`#${id}`);
      expect(button.getAttribute('aria-label'), id).toBeTruthy();
      expect(button.textContent?.trim(), id).toBe('');
      expect(button.querySelector('[data-slot="leadingIcon"]'), id).not.toBeNull();
    }
  });

  // RB-080-86: the colour from a swatch by keyboard or from Nuxt UI's colour picker.
  it('takes the colour from a swatch', async () => {
    await click('#color');
    const blue = el<HTMLButtonElement>('[data-swatch="blue"]');
    expect(blue.getAttribute('aria-label')).toBe('Blue');
    expect(blue.getAttribute('aria-pressed')).toBe('false');
    blue.click();
    await settle();
    expect(el('#color').dataset.value).toBe('#1e88e5');
    key(document.body, 'Escape', { bubbles: true });
    await settle();
    canvas().focus();
    await keyboardRect();
    key(document.body, 's', { ctrlKey: true });
    expect(lastCall(sent, 'annotate')?.args?.annotations).toEqual([expect.objectContaining({ type: 'rect', color: '#1e88e5' })]);
  });

  // RB-080-63: `Failed: plugins.error.too_large` - a host code in the plugin's words.
  it('says a host error in words, never as its key', async () => {
    await keyboardRect();
    await click('#save');
    await answer(sent, 'annotate', 'plugins.error.too_large', false);
    expect(status()).toBe('Failed: The data was larger than one panel message may carry (64 KiB).');
    await click('#save');
    await answer(sent, 'annotate', 'plugins.error.unheard_of', false);
    expect(status()).toBe('Failed: The plugin could not do that.'); // RB-080-90: never the code
  });

  it('fetches the first chunk with the hash when the first answer had no room for it', async () => {
    fromHost({ type: 'event', message: { open: SHOT } });
    await settle();
    const marker = { type: 'marker', x: 1, y: 1, number: 1, color: '#000000', size: 12 };
    await answer(sent, 'load', { ...FIRST, data: '', next: 0, annotations: [marker] });
    expect(lastCall(sent, 'load')?.args).toEqual({ image: SHOT, offset: 0, hash: 'h1' });
    await answer(sent, 'load', { image: SHOT, offset: 0, next: null, hash: 'h1', data: btoa('png') });
    expect(listed()).toEqual(['Number']);
    expect(status()).toBe(`${SHOT}: 100 x 80 px.`);
  });

  it('opens an image without saved annotations too large for it and says so', async () => {
    fromHost({ type: 'event', message: { open: SHOT } });
    await settle();
    await answer(sent, 'load', { ...FIRST, annotations: null, annotations_too_large: true });
    expect(listed()).toEqual([]);
    expect(status()).toContain('larger than the editor takes (64 KiB)');
  });

  it('says when there is no image to open', async () => {
    await click('#refresh');
    await answer(sent, 'list', { images: [], truncated: false });
    expect(el('#no-images').textContent).toContain('No PNG files in .rbrowser/out/ yet.');
    expect(el<HTMLButtonElement>('#open').disabled).toBe(true);
  });
});
