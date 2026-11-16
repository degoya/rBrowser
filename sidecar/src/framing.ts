// Line framing of the stdio protocol: one JSON message per line, UTF-8, `\n` terminated.

/** The longest line the sidecar reads; a longer one is dropped and answered with a parse error. */
export const MAX_LINE_BYTES = 16 * 1024 * 1024;

/**
 * Splits a byte stream into lines. A line longer than `maxBytes` is never held in memory: its
 * bytes are skipped up to the next newline and `onOverflow` gets their count.
 */
export class LineSplitter {
  #chunks: Buffer[] = [];
  #length = 0;
  #skipping = false;
  #skipped = 0;
  readonly #maxBytes: number;
  readonly #onLine: (line: string) => void;
  readonly #onOverflow: (bytes: number) => void;

  constructor(maxBytes: number, onLine: (line: string) => void, onOverflow: (bytes: number) => void) {
    this.#maxBytes = maxBytes;
    this.#onLine = onLine;
    this.#onOverflow = onOverflow;
  }

  push(chunk: Buffer): void {
    let start = 0;
    while (start < chunk.length) {
      const newline = chunk.indexOf(0x0a, start);
      const end = newline === -1 ? chunk.length : newline;
      this.#take(chunk.subarray(start, end));
      if (newline === -1) {
        return;
      }
      this.#finish();
      start = newline + 1;
    }
  }

  #take(part: Buffer): void {
    if (this.#skipping) {
      this.#skipped += part.length;
      return;
    }
    if (this.#length + part.length > this.#maxBytes) {
      this.#skipping = true;
      this.#skipped = this.#length + part.length;
      this.#chunks = [];
      this.#length = 0;
      return;
    }
    this.#chunks.push(part);
    this.#length += part.length;
  }

  #finish(): void {
    if (this.#skipping) {
      const skipped = this.#skipped;
      this.#skipping = false;
      this.#skipped = 0;
      this.#onOverflow(skipped);
      return;
    }
    let line = Buffer.concat(this.#chunks, this.#length).toString('utf8');
    this.#chunks = [];
    this.#length = 0;
    if (line.endsWith('\r')) {
      line = line.slice(0, -1);
    }
    this.#onLine(line);
  }
}
