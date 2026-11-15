// The designs' small previews (RB-080-74): the one the component kept for a design version, else
// drawn here once from the design's bytes (which the component checked, served in chunks) and
// handed to the component to keep. One design at a time; a preview that cannot be drawn is left
// out for the panel's life, the name and size stand alone. The decoded image and the whole file
// are dropped either way.
import { shallowReactive } from 'vue';
import type { Design } from './model';

/** The preview's width in image pixels (twice its CSS width) and its tallest height. */
const THUMB_WIDTH = 96;
const THUMB_HEIGHT = 128;
/** The largest preview the component keeps (32 KiB), as base64. */
const MAX_THUMB_BASE64 = 43692;

type Designs = (args: Record<string, unknown>) => Promise<unknown>;

/** Draws `binary` as a small PNG, as base64; null when it cannot. */
async function thumbnail(binary: string, design: Design): Promise<string | null> {
  try {
    const image = new Image();
    image.src = `data:${design.mime};base64,${btoa(binary)}`;
    await image.decode();
    const canvas = document.createElement('canvas');
    const context = canvas.getContext('2d');
    const width = design.width ?? 0;
    const height = design.height ?? 0;
    if (!context || width <= 0 || height <= 0) return null;
    const scaled = Math.max(1, Math.round((height * THUMB_WIDTH) / width));
    canvas.width = THUMB_WIDTH;
    canvas.height = Math.min(scaled, THUMB_HEIGHT);
    // The top of a tall design: where a page starts.
    context.drawImage(image, 0, 0, THUMB_WIDTH, scaled);
    const url = canvas.toDataURL('image/png');
    const prefix = 'data:image/png;base64,';
    const data = url.startsWith(prefix) ? url.slice(prefix.length) : '';
    return data.length > 0 && data.length <= MAX_THUMB_BASE64 ? data : null;
  } catch {
    return null;
  }
}

export interface Previews {
  /** The preview of a design version as a `data:` URL; undefined while loading or when none. */
  url(hash: string | undefined): string | undefined;
  /** Loads the preview of `design`, once per version. */
  request(design: Design): void;
}

/** The previews, asked from the tool `designs` through `designs`. */
export function createPreviews(designs: Designs): Previews {
  /** By hash: a `data:` URL; `''` while loading or when none could be drawn. */
  const urls = shallowReactive(new Map<string, string>());
  let queue: Promise<void> = Promise.resolve();

  async function load(design: Design, hash: string): Promise<void> {
    try {
      const kept = (await designs({ thumb: hash })) as { data?: unknown } | null;
      if (typeof kept?.data === 'string' && kept.data.length > 0) {
        urls.set(hash, `data:image/png;base64,${kept.data}`);
        return;
      }
      let binary = '';
      for (let index = 0; index < (design.chunks ?? 0); index++) {
        const answer = (await designs({ path: design.path, hash, chunk: index })) as { data?: unknown } | null;
        binary += atob(String(answer?.data ?? ''));
      }
      const data = await thumbnail(binary, design);
      if (data === null) return;
      urls.set(hash, `data:image/png;base64,${data}`);
      designs({ thumb: hash, data }).catch(() => undefined);
    } catch {
      // No preview: the name and size stand alone.
    }
  }

  return {
    url: (hash) => (hash ? urls.get(hash) || undefined : undefined),
    request(design) {
      const hash = design.hash;
      if (!hash || !design.chunks || urls.has(hash)) return;
      urls.set(hash, '');
      queue = queue.then(() => load(design, hash));
    },
  };
}
