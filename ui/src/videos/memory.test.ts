import { describe, expect, it, vi } from 'vitest';
import type { VideoFile } from '../ipc/protocol';
import { MEMORY_LIMIT, MemoryCopies, Released, TooLarge, failureOf, playsFromMemory, type MemoryHost } from './memory';

const file = (name: string, bytes = 1000): VideoFile => ({
  name,
  breakpoint: name.replace(/\.[^.]+$/, ''),
  bytes,
  path: `C:/shop/.rbrowser/out/video/r/${name}`,
  url: `rbrowser://video/2026-11-16_14-03-07/${name}`,
});

function host(answer: () => Promise<Response> = async () => new Response(new Blob(['webm']), { status: 200 })) {
  let next = 0;
  const made: string[] = [];
  const revoked: string[] = [];
  const fetched: { url: string; init: RequestInit }[] = [];
  const fake: MemoryHost = {
    fetch: vi.fn(async (url: string, init: RequestInit) => {
      fetched.push({ url, init });
      return answer();
    }),
    createObjectURL: () => {
      next += 1;
      const url = `blob:rbrowser://ui/${next}`;
      made.push(url);
      return url;
    },
    revokeObjectURL: (url) => {
      revoked.push(url);
    },
  };
  return { fake, made, revoked, fetched };
}

describe('the player error', () => {
  it("keeps Chromium's text and the code", () => {
    expect(failureOf({ code: 4, message: 'DEMUXER_ERROR_COULD_NOT_OPEN: FFmpegDemuxer: open context failed' })).toEqual({
      code: 4,
      detail: 'DEMUXER_ERROR_COULD_NOT_OPEN: FFmpegDemuxer: open context failed',
    });
    expect(failureOf({ code: 2, message: '  PIPELINE_ERROR_NETWORK\n' }).detail).toBe('PIPELINE_ERROR_NETWORK');
  });

  it("names the code when there is no text, and cuts a long one", () => {
    expect(failureOf({ code: 3, message: '' })).toEqual({ code: 3, detail: 'MEDIA_ERR_DECODE' });
    expect(failureOf({ code: 4 })).toEqual({ code: 4, detail: 'MEDIA_ERR_SRC_NOT_SUPPORTED' });
    expect(failureOf(null)).toEqual({ code: 0, detail: 'MEDIA_ERR_UNKNOWN' });
    expect(failureOf({ code: 4, message: 'x'.repeat(1000) }).detail).toHaveLength(300);
  });

  it('loads a WebM whole after a failure, never an MP4', () => {
    expect(playsFromMemory('ipad-mini.webm')).toBe(true);
    expect(playsFromMemory('ipad-mini.mp4')).toBe(false);
    expect(playsFromMemory('ipad-mini.MP4')).toBe(false);
  });
});

describe('a recording in memory', () => {
  it('fetches a video once, whole, and plays it from an object URL', async () => {
    const { fake, fetched, made } = host();
    const copies = new MemoryCopies(fake);
    const url = await copies.load(file('ipad-mini.webm'));
    expect(url).toBe('blob:rbrowser://ui/1');
    expect(fetched).toEqual([{ url: 'rbrowser://video/2026-11-16_14-03-07/ipad-mini.webm', init: { cache: 'no-store' } }]);
    expect(fetched[0]?.init).not.toHaveProperty('headers');
    expect(await copies.load(file('ipad-mini.webm'))).toBe(url);
    expect(fetched).toHaveLength(1);
    expect(made).toEqual([url]);
    expect(copies.bytes).toBe(1000);
  });

  it('holds at most the limit per recording', async () => {
    const { fake, fetched } = host();
    const copies = new MemoryCopies(fake, 2500);
    await copies.load(file('a.webm', 1000));
    await copies.load(file('b.webm', 1000));
    await expect(copies.load(file('c.webm', 1000))).rejects.toBeInstanceOf(TooLarge);
    expect(fetched).toHaveLength(2);
    await expect(copies.load(file('d.webm', 500))).resolves.toBe('blob:rbrowser://ui/3');
    expect(new MemoryCopies(fake).bytes).toBe(0);
    expect(MEMORY_LIMIT).toBe(512 * 1024 * 1024);
  });

  it('gives the room back when a load fails', async () => {
    const { fake } = host(async () => new Response('', { status: 404 }));
    const copies = new MemoryCopies(fake, 1500);
    await expect(copies.load(file('a.webm', 1000))).rejects.toThrow('404');
    expect(copies.bytes).toBe(0);
  });

  it('revokes every copy when the recording is left, and makes none of one that arrives later', async () => {
    let arrive: (response: Response) => void = () => undefined;
    const slow = new Promise<Response>((resolve) => {
      arrive = resolve;
    });
    let first = true;
    const { fake, revoked } = host(async () => {
      if (first) {
        first = false;
        return new Response(new Blob(['a']), { status: 200 });
      }
      return slow;
    });
    const copies = new MemoryCopies(fake);
    const held = await copies.load(file('a.webm'));
    const pending = copies.load(file('b.webm'));
    copies.release();
    expect(revoked).toEqual([held]);
    arrive(new Response(new Blob(['b']), { status: 200 }));
    await expect(pending).rejects.toBeInstanceOf(Released);
    await expect(copies.load(file('c.webm'))).rejects.toBeInstanceOf(Released);
    expect(copies.bytes).toBe(0);
  });
});
