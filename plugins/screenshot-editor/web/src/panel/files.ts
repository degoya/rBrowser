// The screenshot editor's image in and out (RB-080-01, RB-080-63): the chunks of the tool `load`
// put together, the edited image with the crop applied for the clipboard, and the image list as
// the file select's groups (RB-080-86: one group per folder, so a long list stays scannable).
import { clip, cropOf, render } from './draw';
import type { ShotAnnotation, ShotImage } from './types';

/** The largest image the panel loads (src/cache.rs `MAX_BYTES`, RB-099-24): an image too large
 * for a canvas comes as its scaled preview, so this bounds a preview or an image of at most 25
 * megapixels. */
export const MAX_BYTES = 128 * 1024 * 1024;
/** The most chunks one image takes: `MAX_BYTES` in 47 KiB chunks, and room. */
export const MAX_CHUNKS = 2800;

/** One answer of the tool `load`. */
export interface LoadAnswer {
  image?: unknown;
  width?: unknown;
  height?: unknown;
  data?: unknown;
  offset?: unknown;
  next?: number | null;
  hash?: unknown;
  annotations?: unknown;
  annotations_too_large?: unknown;
  /** Image pixels per pixel of `data`: more than 1 for a large image's preview. */
  scale?: unknown;
  bytes?: unknown;
}

/**
 * Every chunk of `path` (none: the newest screenshot). The first answer names the size, the
 * saved annotations and the `hash` every later call passes; its data may be empty with `next`
 * 0 when the annotations filled it (src/tools.rs `fit_first`).
 */
export async function fetchImage(call: (name: string, args: Record<string, unknown>) => Promise<unknown>, path: string | null): Promise<{ first: LoadAnswer; blob: Blob }> {
  const parts: BlobPart[] = [];
  const first = ((await call('load', path ? { image: path } : {})) ?? {}) as LoadAnswer;
  const hash = first.hash;
  if (Number(first.bytes ?? 0) > MAX_BYTES) throw new Error('plugin.screenshot-editor.panel.too_many_chunks');
  let answer = first;
  for (let count = 0; ; count += 1) {
    parts.push(Uint8Array.from(atob(String(answer.data ?? '')), (c) => c.charCodeAt(0)));
    if (answer.next === null || answer.next === undefined) break;
    if (count >= MAX_CHUNKS) throw new Error('plugin.screenshot-editor.panel.too_many_chunks');
    answer = ((await call('load', { image: first.image, offset: answer.next, hash })) ?? {}) as LoadAnswer;
    if (answer.hash !== hash) throw new Error('plugin.screenshot-editor.panel.changed');
  }
  return { first, blob: new Blob(parts, { type: 'image/png' }) };
}

/** The open image as `finished` and `copy` need it; `scale` 1 when not named. */
type Shown = Pick<ShotImage, 'bitmap' | 'width' | 'height'> & { scale?: number };

/** The edited image with the crop applied, as a canvas - at the bitmap's resolution, so a large
 * image's preview at 1:scale (RB-099-24); the saved file is rendered at full size. */
export function finished(image: Shown, items: readonly ShotAnnotation[]): HTMLCanvasElement | null {
  const scale = Math.max(1, image.scale ?? 1);
  const full = document.createElement('canvas');
  full.width = Math.ceil(image.width / scale);
  full.height = Math.ceil(image.height / scale);
  const ctx = full.getContext('2d');
  if (!ctx) return null;
  render(ctx, image.bitmap, items, image.width, image.height, scale);
  const crop = cropOf(items);
  const region = crop && clip((crop.x ?? 0) / scale, (crop.y ?? 0) / scale, (crop.w ?? 0) / scale, (crop.h ?? 0) / scale, full.width, full.height);
  if (!region) return full;
  const cut = document.createElement('canvas');
  cut.width = region.right - region.left;
  cut.height = region.bottom - region.top;
  cut.getContext('2d')?.drawImage(full, -region.left, -region.top);
  return cut;
}

/** The edited image on the system clipboard, as a PNG. */
export async function copy(image: Shown, items: readonly ShotAnnotation[]): Promise<void> {
  const done = finished(image, items);
  if (!done) throw new Error('canvas');
  const blob = await new Promise<Blob>((resolve, reject) => done.toBlob((made) => (made ? resolve(made) : reject(new Error('png'))), 'image/png'));
  await navigator.clipboard.write([new ClipboardItem({ 'image/png': blob })]);
}

/** The paths of the answer of `list`. */
export const paths = (images: unknown): string[] => (Array.isArray(images) ? images.map(String) : []);

/** A path as the menu shows it: below `.rbrowser/out/`. */
export const shortName = (path: string): string => path.replace(/^\.rbrowser\/out\//, '');

export type FileItem = { type: 'label'; label: string } | { label: string; value: string };

/**
 * The select's items of `list`'s paths in their order: a label where a folder starts, then its
 * files, each by its name (a screenshot run's breakpoints stand together).
 */
export function fileItems(all: readonly string[]): FileItem[][] {
  const groups: FileItem[][] = [];
  let folder: string | null = null;
  for (const path of all) {
    const short = shortName(path);
    const cut = short.lastIndexOf('/');
    const here = cut < 0 ? '' : short.slice(0, cut);
    if (here !== folder || groups.length === 0) {
      folder = here;
      groups.push(here ? [{ type: 'label', label: here }] : []);
    }
    groups[groups.length - 1]?.push({ label: cut < 0 ? short : short.slice(cut + 1), value: path });
  }
  return groups;
}
