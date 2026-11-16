// The Videos panel's fallback (RB-110-12, acceptance 2026-11-16: no WebM played from the route):
// when the player cannot stream a video from `rbrowser://video/...`, the panel fetches the file
// once, whole (a plain request without a range - the poster's way, which works), and plays it
// from memory through an object URL. At most `MEMORY_LIMIT` bytes per recording; the copies are
// revoked when the recording is left (the player unmounts). An MP4 is never fetched: memory does
// not give rBrowser's Chromium an H.264 decoder.

import type { VideoFile } from '../ipc/protocol';

/** The most bytes of one recording held in memory: 512 MiB. */
export const MEMORY_LIMIT = 512 * 1024 * 1024;
/** The most characters of Chromium's error text kept. */
export const DETAIL_CHARS = 300;

/** The names of `MediaError`'s codes, for an error without Chromium's text. */
const CODE_NAMES: Record<number, string> = {
  1: 'MEDIA_ERR_ABORTED',
  2: 'MEDIA_ERR_NETWORK',
  3: 'MEDIA_ERR_DECODE',
  4: 'MEDIA_ERR_SRC_NOT_SUPPORTED',
};

/** What the player said: `MediaError.code` (0 without one) and its message - Chromium's text such
 * as `DEMUXER_ERROR_COULD_NOT_OPEN: ...` or `PIPELINE_ERROR_NETWORK`, else the code's name. */
export interface MediaFailure {
  code: number;
  detail: string;
}

export function failureOf(error: { code?: number; message?: string } | null | undefined): MediaFailure {
  const code = typeof error?.code === 'number' ? error.code : 0;
  const message = typeof error?.message === 'string' ? error.message.replace(/\s+/g, ' ').trim() : '';
  const detail = message === '' ? (CODE_NAMES[code] ?? 'MEDIA_ERR_UNKNOWN') : message.slice(0, DETAIL_CHARS);
  return { code, detail };
}

/** Whether a video that did not stream is worth loading whole: not an MP4. */
export function playsFromMemory(name: string): boolean {
  return !/\.mp4$/i.test(name);
}

/** The recording's copies would pass `MEMORY_LIMIT`. */
export class TooLarge extends Error {
  constructor(readonly limit: number) {
    super('the video is too large to play from memory');
  }
}

/** The player left the recording while a copy was loading. */
export class Released extends Error {
  constructor() {
    super('the recording was left');
  }
}

/** What the copies need of the page; the browser's by default. */
export interface MemoryHost {
  fetch(url: string, init: RequestInit): Promise<Response>;
  createObjectURL(blob: Blob): string;
  revokeObjectURL(url: string): void;
}

export function browserHost(): MemoryHost {
  return {
    fetch: (url, init) => fetch(url, init),
    createObjectURL: (blob) => URL.createObjectURL(blob),
    revokeObjectURL: (url) => URL.revokeObjectURL(url),
  };
}

/** One recording's videos held in memory, by file name. */
export class MemoryCopies {
  private readonly urls = new Map<string, string>();
  private used = 0;
  private released = false;

  constructor(
    private readonly host: MemoryHost = browserHost(),
    private readonly limit = MEMORY_LIMIT,
  ) {}

  /** Bytes held or reserved. */
  get bytes(): number {
    return this.used;
  }

  /** Loads `file` whole, once, and answers its object URL.
   * @throws TooLarge beyond the recording's limit, Released once left, an Error when it did not load. */
  async load(file: VideoFile): Promise<string> {
    const known = this.urls.get(file.name);
    if (known !== undefined) {
      return known;
    }
    if (this.released) {
      throw new Released();
    }
    if (file.bytes > this.limit - this.used) {
      throw new TooLarge(this.limit);
    }
    this.used += file.bytes;
    try {
      const response = await this.host.fetch(file.url, { cache: 'no-store' });
      if (!response.ok) {
        throw new Error(`the video answered ${response.status}`);
      }
      const blob = await response.blob();
      if (this.released) {
        throw new Released();
      }
      const url = this.host.createObjectURL(blob);
      this.urls.set(file.name, url);
      return url;
    } catch (error) {
      if (!this.released) {
        this.used -= file.bytes;
      }
      throw error;
    }
  }

  /** Revokes every copy; later loads are refused. */
  release(): void {
    this.released = true;
    this.urls.forEach((url) => this.host.revokeObjectURL(url));
    this.urls.clear();
    this.used = 0;
  }
}
