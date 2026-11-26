// The screenshot editor's session (RB-080-01, RB-080-63; on Nuxt UI since RB-080-86): the open
// image, the chosen tool and its options, the undo history, the crop draft, the zoom, and what
// the panel sends - `load` in chunks, `list`, `annotate` (`shot_annotate`; the component renders
// the file) - and the clipboard. The canvas, its pointer and keyboard input and the crop frame
// stay the panel's own (draw.ts, input.ts, crop.ts); the components show this state. Every
// sentence goes to the status line; nothing starts before the texts arrived.
import { computed, reactive } from 'vue';
import type { Panel } from '../../../../web-common/panel-ui/src/panel';
import type { StatusLine, Tone } from '../../../../web-common/panel-ui/src/status';
import { fromSettings, hostReason, options as readOptions, shownOptions, stepped } from './controls';
import * as Crop from './crop';
import { cropFrame, cropOf, cursorMark, clip, draw, render } from './draw';
import { add, build, fits, history as makeHistory } from './editor';
import { copy as copyImage, fetchImage, paths as listPaths } from './files';
import * as Input from './input';
import type { ShotAnnotation, ShotImage, ShotRegion } from './types';

export interface Elements {
  canvas: HTMLCanvasElement;
  frame: HTMLElement;
  stage: HTMLElement;
  cropBar: HTMLElement;
}

export function createSession(panel: Panel) {
  const t = panel.t;
  const history = makeHistory();
  const controls = reactive({ color: '#e53935', width: 4, size: 20, mode: 'pixelate', block: 12, fill: false });
  const state = reactive({
    tool: 'rect',
    items: [] as ShotAnnotation[],
    canUndo: false,
    canRedo: false,
    /** The open image's path and size; the pixels are not reactive. */
    image: null as { path: string; width: number; height: number } | null,
    zoom: 1,
    files: [] as string[],
    listed: false,
    file: undefined as string | undefined,
    textOpen: false,
    text: '',
    crop: null as { size: string; applyable: boolean } | null,
    status: null as StatusLine | null,
  });
  const ready = panel.ready;
  let image: ShotImage | null = null;
  let saved = '[]';
  /** The tool before the crop tool, chosen again once a crop is applied or cancelled. */
  let before = 'rect';
  /** The crop frame being drawn or adjusted; not an annotation before Apply. */
  let draft: ShotAnnotation | null = null;
  let zoomChosen = false;
  let discardArmed = false;
  let progress: ShotAnnotation | null = null;
  let cursor: number[] | null = null;
  let textPoint: number[] | null = null;
  const base = document.createElement('canvas');
  let baseItems: ShotAnnotation[] | null = null;
  let els: Elements | null = null;
  let input: Input.Input | null = null;

  function status(text: string, tone: Tone = 'info'): void {
    if (ready.value) state.status = { tone, text };
  }

  /** A failure as one sentence: the plugin's words for a host code, else the shared sentence. */
  function failed(error: unknown): void {
    const code = error instanceof Error ? error.message : String(error);
    const own = hostReason(code, (key) => (panel.strings.value[`plugin.screenshot-editor.${key}`] === undefined ? undefined : t(key)));
    status(t('panel.failed', { error: own ?? panel.error(error) }), 'error');
  }

  const toolName = (key: string): string => t(`panel.tool.${key}`);
  const options = () => readOptions(controls);
  const shown = computed(() => shownOptions(state.tool, controls.mode));
  /** Remove crop: in the crop tool, while a crop is applied. */
  const removable = computed(() => state.tool === 'crop' && cropOf(state.items) !== null);

  function selectTool(chosen: string): void {
    if (chosen === state.tool) return; // Crop again keeps its draft (review of RB-080-63).
    if (chosen === 'crop') before = state.tool;
    state.tool = chosen;
    draft = applied();
    input?.cancel();
    layout();
  }

  /** A copy of the applied crop while the crop tool is chosen: its draft before a change. */
  function applied(): ShotAnnotation | null {
    const crop = state.tool === 'crop' ? cropOf(history.items) : null;
    return crop ? { ...crop } : null;
  }
  /** Whether the draft differs from the crop as applied: only then Apply or Enter changes it. */
  const changedDraft = (): boolean => draft !== null && !Crop.same(draft, cropOf(history.items));

  /** The applied crop's region while the image shows cropped: any tool but the crop tool. */
  function cropped(): ShotRegion | null {
    const crop = image && state.tool !== 'crop' ? cropOf(history.items) : null;
    return crop && image ? clip(crop.x ?? 0, crop.y ?? 0, crop.w ?? 0, crop.h ?? 0, image.width, image.height) : null;
  }

  /** The frame the crop tool shows: the one being dragged, else the draft. */
  const shownFrame = (): ShotAnnotation | null => (state.tool !== 'crop' ? null : progress?.type === 'crop' ? progress : draft);

  let painting = false;
  /** Asks for one paint in the next frame (review of RB-080-63: a pointer move or an action
   * painted several times, each reading layout); the crop bar's state follows at once. */
  function repaint(): void {
    cropBar();
    if (painting) return;
    painting = true;
    const later = typeof requestAnimationFrame === 'function' ? requestAnimationFrame : (next: () => void) => setTimeout(next, 16);
    later(() => {
      painting = false;
      paint();
    });
  }

  function paint(): void {
    const parts = els;
    const ctx = parts?.canvas.getContext('2d');
    if (!parts || !ctx || !image) return;
    const frame = shownFrame();
    if (frame) Crop.place(parts.cropBar, frame, state.zoom, parts.canvas);
    const items = history.items;
    // The canvases hold the bitmap's pixels: the image's, or a large one's preview at 1:scale
    // (RB-099-24); what is drawn on them is in image pixels.
    const { scale } = image;
    if (baseItems !== items) {
      [base.width, base.height] = [parts.canvas.width, parts.canvas.height];
      const baseCtx = base.getContext('2d');
      if (baseCtx) render(baseCtx, image.bitmap, items, image.width, image.height, scale);
      baseItems = items;
    }
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, parts.canvas.width, parts.canvas.height);
    ctx.drawImage(base, 0, 0);
    ctx.setTransform(1 / scale, 0, 0, 1 / scale, 0, 0);
    if (progress && progress.type !== 'crop') draw(ctx, progress, image.width, image.height, scale);
    if (frame) {
      cropFrame(ctx, [frame], image.width, image.height, scale);
      Crop.drawHandles(ctx, frame, 1 / state.zoom);
    }
    if (cursor) cursorMark(ctx, cursor[0] ?? 0, cursor[1] ?? 0, scale);
    ctx.setTransform(1, 0, 0, 1, 0, 0);
  }

  /** Apply and Cancel at the frame, with its size; Apply waits for the drag to end. */
  function cropBar(): void {
    const frame = image ? shownFrame() : null;
    state.crop = frame && image ? { size: t('panel.crop_size', Crop.size(frame, image.width, image.height)), applyable: frame === draft } : null;
  }

  /** The canvas's size at the zoom, the image cropped to the applied crop outside the crop tool. */
  function layout(): void {
    const parts = els;
    if (image && parts) Crop.view(parts.frame, parts.canvas, cropped(), image, state.zoom);
    repaint();
  }

  /** `value`; `chosen` is the person's zoom, kept after a crop. */
  function setZoom(value: number, chosen = false): void {
    [state.zoom, zoomChosen] = [value, chosen];
    layout();
  }

  function fit(): void {
    if (!image) return;
    const region = cropped();
    const width = region ? region.right - region.left : image.width;
    setZoom(Math.min(1, Math.max((els?.stage.clientWidth ?? 0) - 4, 100) / width));
  }

  const stepZoom = (direction: number): void => setZoom(stepped(state.zoom, direction), true);

  function update(): void {
    state.items = history.items;
    state.canUndo = history.canUndo;
    state.canRedo = history.canRedo;
    // Undo, Redo, Discard and Remove change the crop: the draft follows (review of RB-080-63).
    draft = applied();
    report();
    layout();
  }

  function change(items: ShotAnnotation[]): void {
    discardArmed = false;
    history.push(items);
    update();
  }

  /** The draft as the crop; the crop as applied again changes nothing (no undo step, not dirty). */
  function applyCrop(): void {
    if (!draft || !image) return;
    const size = Crop.size(draft, image.width, image.height);
    if (changedDraft()) change(add(history.items, draft));
    selectTool(before);
    input?.reset();
    if (!zoomChosen) fit();
    status(t('panel.crop_applied', size), 'ok');
    els?.canvas.focus();
  }

  function cancelCrop(): void {
    selectTool(before);
    status(t('panel.crop_cancelled'));
    els?.canvas.focus();
  }

  function removeCrop(): void {
    change(history.items.filter((item) => item.type !== 'crop'));
    status(t('panel.crop_removed'));
  }

  function remove(index: number): void {
    change(history.items.filter((_, at) => at !== index));
    els?.canvas.focus();
  }

  function closeText(): void {
    state.textOpen = false;
    textPoint = null;
    els?.canvas.focus();
  }

  function addText(): void {
    const point = textPoint;
    const annotation = point && build('text', [point], options(), history.items, state.text);
    closeText();
    if (annotation) {
      change(add(history.items, annotation));
      status(t('panel.added', { name: toolName(annotation.type) }));
    }
  }

  /** Wires the canvas: the crop frame first, so a grab on it goes no further. */
  function attach(parts: Elements): void {
    els = parts;
    const { canvas } = parts;
    Crop.attach(canvas, {
      active: () => state.tool === 'crop' && image !== null,
      frame: () => draft,
      pending: changedDraft,
      idle: () => !input?.busy,
      change(frame) {
        draft = frame;
        repaint();
      },
      apply: applyCrop,
    });
    input = Input.attach(canvas, {
      tool: () => state.tool,
      options,
      items: () => history.items,
      commit(annotation) {
        if (annotation.type === 'crop') {
          draft = annotation;
          repaint();
          status(t('panel.crop_draft'));
          return;
        }
        change(add(history.items, annotation));
        status(t('panel.added', { name: toolName(annotation.type) }));
      },
      preview(annotation, at) {
        progress = annotation;
        cursor = at;
        repaint();
      },
      // The keyboard cursor starts in the middle of what shows (review of RB-080-63).
      start: () => Crop.middle(cropped() ?? { left: 0, top: 0, right: image?.width ?? canvas.width, bottom: image?.height ?? canvas.height }),
      askText(point) {
        textPoint = point;
        state.text = '';
        state.textOpen = true;
      },
      status: (key) => status(t(key)),
    });
  }

  /** Every chunk of `path` (none: the newest screenshot), then the editor shows it. */
  async function load(path: string | null): Promise<void> {
    status(t('panel.loading', { image: path ?? '' }), 'busy');
    try {
      const { first, blob } = await fetchImage((name, args) => panel.call({ tool: name }, args), path);
      const bitmap = await createImageBitmap(blob);
      const scale = Math.max(1, Math.round(Number(first.scale) || 1));
      image = { path: String(first.image), bitmap, width: Number(first.width), height: Number(first.height), scale };
      const canvas = els?.canvas;
      if (canvas) {
        // The bitmap's size: a large image's preview is `scale` times smaller than the image.
        [canvas.width, canvas.height] = scale > 1 ? [bitmap.width, bitmap.height] : [image.width, image.height];
        [canvas.dataset.width, canvas.dataset.height] = [String(image.width), String(image.height)];
      }
      state.image = { path: image.path, width: image.width, height: image.height };
      baseItems = null;
      const annotations = Array.isArray(first.annotations) ? (first.annotations as ShotAnnotation[]) : [];
      history.reset(annotations);
      saved = JSON.stringify(annotations);
      discardArmed = false;
      state.file = image.path;
      selectTool(state.tool === 'crop' ? before : state.tool);
      fit();
      update();
      if (first.annotations_too_large === true) status(t('panel.annotations_too_large'), 'warn');
      else if (scale > 1) status(t('panel.loaded_scaled', { image: image.path, width: image.width, height: image.height, scale }));
      else status(t('panel.loaded', { image: image.path, width: image.width, height: image.height }));
    } catch (error) {
      failed(error);
    }
  }

  async function refreshList(): Promise<void> {
    try {
      const answer = (await panel.call({ tool: 'list' }, {})) as { images?: unknown } | null;
      state.files = listPaths(answer?.images);
      state.listed = true;
      const keep = image?.path ?? state.file;
      state.file = keep && state.files.includes(keep) ? keep : state.files[0];
    } catch (error) {
      failed(error);
    }
  }

  const dirty = (): boolean => JSON.stringify(history.items) !== saved;
  let reported = false;
  /** Tells the window whether there is unsaved work, so a switch keeps this frame (RB-080-59). */
  function report(): void {
    const now = image !== null && dirty();
    if (now !== reported) panel.post({ type: 'dirty', dirty: (reported = now) });
  }

  /** Opens `path`, after a second press when there are unsaved changes. */
  function open(path: string | null): void {
    if (image && dirty() && !discardArmed) {
      discardArmed = true;
      status(t('panel.discard_confirm'), 'warn');
      return;
    }
    discardArmed = false;
    void load(path);
  }

  async function save(): Promise<void> {
    if (!image) return status(t('panel.no_image'), 'error');
    const args = { image: image.path, annotations: history.items };
    if (!fits(args)) return status(t('panel.too_large'), 'error');
    try {
      const answer = (await panel.call({ tool: 'annotate' }, args)) as { path?: unknown } | null;
      saved = JSON.stringify(history.items);
      report();
      status(t('panel.saved', { path: String(answer?.path ?? '') }), 'ok');
    } catch (error) {
      failed(error);
    }
  }

  async function copy(): Promise<void> {
    if (!image) return status(t('panel.no_image'), 'error');
    try {
      await copyImage(image, history.items);
      // A large image is copied as the editor shows it; the saved file has its full size.
      if (image.scale > 1) status(t('panel.copied_scaled', { scale: image.scale }), 'ok');
      else status(t('panel.copied'), 'ok');
    } catch (error) {
      status(t('panel.copy_failed', { error: (error as Error | null)?.message ?? '' }), 'error');
    }
  }

  function revert(): void {
    if (!dirty()) return;
    history.push(JSON.parse(saved) as ShotAnnotation[]);
    discardArmed = false;
    update();
    status(t('panel.reverted'));
  }

  function undo(): void {
    history.undo();
    update();
  }

  function redo(): void {
    history.redo();
    update();
  }

  /** Escape outside a field: a crop draft first; else what the tooltip says, and a clean crop
   * tool is left. */
  function escape(): void {
    if (input?.busy) input.cancel();
    else if (state.tool === 'crop' && changedDraft()) cancelCrop();
    else if (dirty() || state.tool !== 'crop') revert();
    else cancelCrop();
  }

  /** The tool of a letter key, with the status saying so. */
  function chooseByKey(chosen: string): void {
    selectTool(chosen);
    status(t(chosen === 'crop' ? 'panel.crop_hint' : 'panel.tool_chosen', { name: toolName(chosen) }));
  }

  /** The plugin settings' starting values (every `settings` message, as before RB-080-86). */
  function applySettings(settings: Record<string, unknown>): void {
    Object.assign(controls, fromSettings(settings));
    update();
  }

  /** Repaints with the annotation font once it is there. */
  function fontLoaded(): void {
    baseItems = null;
    repaint();
  }

  return {
    state,
    controls,
    shown,
    removable,
    attach,
    selectTool,
    chooseByKey,
    undo,
    redo,
    save,
    copy,
    revert,
    escape,
    open,
    load,
    refreshList,
    applyCrop,
    cancelCrop,
    removeCrop,
    remove,
    addText,
    closeText,
    stepZoom,
    fit,
    applySettings,
    fontLoaded,
    layout,
    toolName,
  };
}

export type Session = ReturnType<typeof createSession>;
