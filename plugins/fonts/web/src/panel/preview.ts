// The Fonts panel's previews (RB-080-83, the owner's wish of 2026-11-15): each family and face
// set in its own font. The panel has no network, so a web font's file comes from the plugin's
// tool `preview` - read again in a view that loaded it, part by part under the panel message
// limit (64 KiB, RB-080-63) - and is loaded with `new FontFace(name, bytes)`; nothing is fetched
// from the frame. An installed or generic family is shown by its name. Where a preview cannot be
// had, the state says why. One file is read at a time, each once.

import { reactive } from 'vue';
import type { Answer, FaceEntry, Family } from './model';

export type Reason = 'too_large' | 'not_listed' | 'unreadable' | 'no_file' | 'not_loaded' | 'failed';

export type PreviewState =
  | { kind: 'ready'; css: string }
  | { kind: 'loading' }
  | { kind: 'missing'; reason: Reason; kb?: number };

/** One answer of the tool `preview`. */
export interface PreviewPart {
  src: string;
  offset?: number;
  total?: number;
  data?: string;
  done?: boolean;
  error?: string;
  maxKb?: number;
}

/** What loading needs, given from outside so the tests can stand in for the browser. */
export interface PreviewDeps {
  /** The tool `preview`. */
  read(view: number, src: string, offset: number): Promise<PreviewPart>;
  /** Loads `source` as `family` (`new FontFace(...).load()` and `document.fonts.add`). */
  load(family: string, source: ArrayBuffer | string, descriptors: { weight: string; style: string }): Promise<void>;
}

const GENERIC = new Set([
  'serif', 'sans-serif', 'monospace', 'cursive', 'fantasy', 'system-ui', 'ui-serif', 'ui-sans-serif', 'ui-monospace', 'ui-rounded',
  'emoji', 'math', 'fangsong',
]);
const REASONS: readonly string[] = ['too_large', 'not_listed', 'unreadable'];
/** The most parts read for one file: 4 MiB, the setting's top, in parts of 40 KiB. */
const MAX_PARTS = 128;

/** The CSS `font-family` of a family shown by its name: a generic keyword as it is, a name quoted. */
export function cssFamily(name: string): string {
  return GENERIC.has(name.toLowerCase()) ? name : `"${name.replace(/["\\]/g, (c) => `\\${c}`)}"`;
}

/** The first Chromium view that loaded `face` (only Chromium views evaluate), or null. */
export function sourceView(answer: Answer, face: FaceEntry): number | null {
  const chromium = new Set(answer.views.filter((view) => view.engine === 'chromium' && !view.error).map((view) => view.id));
  return face.status.find((entry) => entry.state === 'loaded' && chromium.has(entry.view))?.view ?? null;
}

/** The face a family's one-line preview uses: regular if it has one, else its first. */
export function mainFace(family: Family): FaceEntry | undefined {
  return family.faces.find((face) => face.weight === '400' && face.style === 'normal') ?? family.faces[0];
}

function decode(base64: string): Uint8Array {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let at = 0; at < binary.length; at += 1) bytes[at] = binary.charCodeAt(at);
  return bytes;
}

/** The previews of one panel. */
export class FontPreviews {
  private readonly states = reactive(new Map<string, PreviewState>());
  private queue: Promise<void> = Promise.resolve();
  private next = 0;

  constructor(private readonly deps: PreviewDeps) {}

  private static key(face: FaceEntry): string {
    return `${face.src ?? ''}|${face.weight}|${face.style}`;
  }

  /** How `face` of `family` is shown now; `undefined` before `request`. */
  state(family: Family, face: FaceEntry | undefined): PreviewState | undefined {
    if (!family.webFont) return { kind: 'ready', css: cssFamily(family.family) };
    if (!face) return { kind: 'missing', reason: 'no_file' };
    return this.states.get(FontPreviews.key(face));
  }

  /** Starts loading every face of `families` not asked for yet. */
  request(answer: Answer, families: Family[]): void {
    for (const family of families) {
      if (!family.webFont) continue;
      for (const face of family.faces) {
        const key = FontPreviews.key(face);
        if (this.states.has(key)) continue;
        this.states.set(key, { kind: 'loading' });
        this.queue = this.queue.then(() => this.fetch(answer, face).then((state) => void this.states.set(key, state)));
      }
    }
  }

  /** Loads one face; its state when done. */
  private async fetch(answer: Answer, face: FaceEntry): Promise<PreviewState> {
    const src = face.src ?? '';
    const name = `rb-preview-${(this.next += 1)}`;
    const descriptors = { weight: face.weight || 'normal', style: face.style || 'normal' };
    try {
      if (src.startsWith('data:')) {
        await this.deps.load(name, `url("${src.replace(/"/g, '%22')}")`, descriptors);
        return { kind: 'ready', css: `"${name}"` };
      }
      if (!/^https?:\/\//.test(src)) return { kind: 'missing', reason: 'no_file' };
      const view = sourceView(answer, face);
      if (view === null) return { kind: 'missing', reason: 'not_loaded' };
      const parts: Uint8Array[] = [];
      let offset = 0;
      for (let count = 0; count < MAX_PARTS; count += 1) {
        const part = await this.deps.read(view, src, offset);
        if (part.error) {
          const reason = (REASONS.includes(part.error) ? part.error : 'unreadable') as Reason;
          return { kind: 'missing', reason, ...(part.maxKb === undefined ? {} : { kb: part.maxKb }) };
        }
        const bytes = decode(part.data ?? '');
        parts.push(bytes);
        offset += bytes.length;
        if (part.done || bytes.length === 0) break;
      }
      const file = new Uint8Array(offset);
      let at = 0;
      for (const bytes of parts) {
        file.set(bytes, at);
        at += bytes.length;
      }
      await this.deps.load(name, file.buffer, descriptors);
      return { kind: 'ready', css: `"${name}"` };
    } catch {
      return { kind: 'missing', reason: 'failed' };
    }
  }
}

/** The browser's way to load a font: `FontFace` and `document.fonts`. */
export async function browserLoad(family: string, source: ArrayBuffer | string, descriptors: { weight: string; style: string }): Promise<void> {
  const face = new FontFace(family, source, descriptors);
  await face.load();
  document.fonts.add(face);
}
