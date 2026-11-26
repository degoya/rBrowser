import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Agent } from './agent';
import { MAX_MEDIA_REPORTS, cleanSrc, watchMedia } from './media';
import type { AgentMessage, MediaFailure } from './protocol';
import { el, loadPage } from './test-page';

/** A build without the proprietary codecs: MP4 with H.264, AAC and HEVC is refused. */
function standardBuild(type: string): CanPlayTypeResult {
  return /avc1|mp4a|hvc1|quicktime/.test(type) ? '' : 'maybe';
}

function failWith(element: HTMLMediaElement, code: number, message: string): void {
  Object.defineProperty(element, 'error', { configurable: true, value: { code, message } });
  element.dispatchEvent(new Event('error'));
}

beforeEach(() => {
  vi.spyOn(HTMLMediaElement.prototype, 'canPlayType').mockImplementation(standardBuild);
  loadPage(
    '<!doctype html><html><body><main>' +
      '<video id="clip" src="https://cdn.test/media/clip.mp4?token=abc#t=2"></video>' +
      '<video id="multi"><source id="h264" src="/media/clip.mp4" type=\'video/mp4; codecs="avc1.42E01E"\'>' +
      '<source id="webm" src="/media/clip.webm" type="video/webm"></video>' +
      '<audio id="song" src="/media/song.opus"></audio>' +
      '</main></body></html>',
  );
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('cleanSrc', () => {
  it('drops the query and the fragment of an address', () => {
    expect(cleanSrc('https://cdn.test/a/clip.mp4?token=abc#t=2')).toBe('https://cdn.test/a/clip.mp4');
  });

  it('keeps only the media type of a data URL', () => {
    expect(cleanSrc('data:video/mp4;base64,AAAA')).toBe('data:video/mp4');
    expect(cleanSrc('data:,x')).toBe('data:');
  });

  it('cuts a long address', () => {
    expect(cleanSrc(`https://cdn.test/${'a'.repeat(5000)}.mp4`).length).toBe(2048);
  });
});

describe('watchMedia', () => {
  it('reports a media element that failed with MEDIA_ERR_SRC_NOT_SUPPORTED', () => {
    const seen: MediaFailure[] = [];
    const stop = watchMedia(window, (failure) => seen.push(failure));
    failWith(el<HTMLVideoElement>('#clip'), 4, 'DEMUXER_ERROR_NO_SUPPORTED_STREAMS: FFmpegDemuxer: no supported streams');
    stop();
    expect(seen).toEqual([
      {
        url: window.location.href.split(/[?#]/)[0],
        tag: 'video',
        code: 4,
        message: 'DEMUXER_ERROR_NO_SUPPORTED_STREAMS: FFmpegDemuxer: no supported streams',
        src: 'https://cdn.test/media/clip.mp4',
        plays: { h264: false, aac: false, hevc: false },
        path: expect.stringContaining('#clip'),
      },
    ]);
  });

  it('ignores the other media errors (aborted, network, decode)', () => {
    const seen: MediaFailure[] = [];
    const stop = watchMedia(window, (failure) => seen.push(failure));
    for (const code of [1, 2, 3]) failWith(el<HTMLVideoElement>('#clip'), code, 'x');
    stop();
    expect(seen).toEqual([]);
  });

  it('reports a source with the type and what the build says to it', () => {
    const seen: MediaFailure[] = [];
    const stop = watchMedia(window, (failure) => seen.push(failure));
    el('#h264').dispatchEvent(new Event('error'));
    stop();
    expect(seen).toHaveLength(1);
    expect(seen[0]).toMatchObject({
      tag: 'source',
      type: 'video/mp4; codecs="avc1.42E01E"',
      canPlay: '',
      plays: { h264: false, aac: false, hevc: false },
      path: expect.stringContaining('#h264'),
    });
    expect(seen[0]?.src).toMatch(/\/media\/clip\.mp4$/);
    expect(seen[0]?.code).toBeUndefined();
  });

  it('reports each source once per document and stops at the limit', () => {
    const seen: MediaFailure[] = [];
    const stop = watchMedia(window, (failure) => seen.push(failure));
    el('#h264').dispatchEvent(new Event('error'));
    el('#h264').dispatchEvent(new Event('error'));
    expect(seen).toHaveLength(1);
    const video = el<HTMLVideoElement>('#multi');
    for (let index = 0; index < MAX_MEDIA_REPORTS + 5; index += 1) {
      const source = document.createElement('source');
      source.src = `/media/other-${index}.mp4`;
      video.append(source);
      source.dispatchEvent(new Event('error'));
    }
    stop();
    expect(seen).toHaveLength(MAX_MEDIA_REPORTS);
  });

  it('ignores an error of anything else and after it stopped', () => {
    const seen: MediaFailure[] = [];
    const stop = watchMedia(window, (failure) => seen.push(failure));
    const image = document.createElement('img');
    document.body.append(image);
    image.dispatchEvent(new Event('error'));
    stop();
    failWith(el<HTMLVideoElement>('#clip'), 4, 'MEDIA_ELEMENT_ERROR: Format error');
    expect(seen).toEqual([]);
  });

  it('is reported by the agent of every frame as a media message', () => {
    const sent: AgentMessage[] = [];
    const agent = new Agent({ win: window, send: (payload) => sent.push(JSON.parse(payload)) });
    agent.start();
    failWith(el<HTMLAudioElement>('#song'), 4, 'MEDIA_ELEMENT_ERROR: Format error');
    agent.stop();
    failWith(el<HTMLVideoElement>('#clip'), 4, 'MEDIA_ELEMENT_ERROR: Format error');
    const media = sent.filter((message) => message.kind === 'media');
    expect(media).toHaveLength(1);
    expect(media[0]).toMatchObject({ kind: 'media', v: 1, top: true, tag: 'audio', code: 4 });
  });
});
