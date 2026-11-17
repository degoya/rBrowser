import { describe, expect, it } from 'vitest';
import type { VideoRecording } from '../ipc/protocol';
import {
  SPEEDS,
  conversionOf,
  conversionText,
  deleteQuestion,
  durationText,
  groupByDay,
  mediaBox,
  memoryNotice,
  offerFor,
  playError,
  recordingPoster,
  recordingTime,
  sizeText,
  tooLargeError,
  viewsOf,
} from './videos';

const recording = (id: string, views = 1): VideoRecording => ({
  id,
  path: `/p/${id}`,
  files: Array.from({ length: views }, (_, index) => ({
    name: `v${index}.mp4`,
    breakpoint: `v${index}`,
    bytes: 1,
    path: `/p/${id}/v${index}.mp4`,
    url: `rbrowser://video/${id}/v${index}.mp4`,
  })),
});

describe('the recordings list', () => {
  it('groups by day in the order the core sent (newest first)', () => {
    const groups = groupByDay([
      recording('2026-11-16_14-03-07'),
      recording('2026-11-16_09-00-00'),
      recording('2026-11-15_23-59-59'),
      recording('odd-name'),
    ]);
    expect(groups.map((group) => [group.day, group.recordings.map((entry) => entry.id)])).toEqual([
      ['2026-11-16', ['2026-11-16_14-03-07', '2026-11-16_09-00-00']],
      ['2026-11-15', ['2026-11-15_23-59-59']],
      ['odd-name', ['odd-name']],
    ]);
  });

  it('names a recording by its time of day', () => {
    expect(recordingTime('2026-11-16_14-03-07')).toBe('14:03:07');
    expect(recordingTime('odd-name')).toBe('odd-name');
  });

  it('writes sizes and durations the way people read them', () => {
    expect(sizeText(512)).toEqual({ key: 'videos.size.bytes', args: { size: 512 } });
    expect(sizeText(1536)).toEqual({ key: 'videos.size.kb', args: { size: '1.5' } });
    expect(sizeText(5 * 1024 * 1024)).toEqual({ key: 'videos.size.mb', args: { size: '5.0' } });
    expect(sizeText(3 * 1024 ** 3)).toEqual({ key: 'videos.size.gb', args: { size: '3.0' } });
    expect(durationText(0)).toBe('0:00');
    expect(durationText(42.6)).toBe('0:42');
    expect(durationText(62)).toBe('1:02');
    expect(durationText(3723)).toBe('1:02:03');
    expect(durationText(Number.NaN)).toBe('');
    expect(durationText(Number.POSITIVE_INFINITY)).toBe('');
  });

  it('offers the speeds as a choice, normal speed among them', () => {
    expect(SPEEDS).toContain(1);
    expect([...SPEEDS].sort((a, b) => a - b)).toEqual([...SPEEDS]);
  });

  it('asks a deletion question that says how many files go and what stays', () => {
    expect(deleteQuestion(recording('2026-11-16_14-03-07', 3), null)).toEqual({
      key: 'videos.delete.recording',
      args: { time: '14:03:07', count: 3 },
    });
    expect(deleteQuestion(recording('2026-11-16_14-03-07', 3), 'v1.mp4')).toEqual({
      key: 'videos.delete.file',
      args: { name: 'v1.mp4', count: 2 },
    });
  });

  it('shows a recording by the poster of its first view that has one', () => {
    const plain = recording('2026-11-16_14-03-07', 2);
    expect(recordingPoster(plain)).toBeUndefined();
    const second = plain.files[1];
    if (second === undefined) {
      throw new Error('two views');
    }
    second.poster = 'rbrowser://video/2026-11-16_14-03-07/v1.jpg';
    expect(recordingPoster(plain)).toBe('rbrowser://video/2026-11-16_14-03-07/v1.jpg');
  });

  it('says why a video does not play, with the player\'s code and text: an MP4 is a format rBrowser cannot decode', () => {
    // The format setting is gone (RB-090-52): the text points to "Convert to WebM".
    const unsupported = { code: 4, detail: 'DEMUXER_ERROR_NO_SUPPORTED_STREAMS' };
    expect(playError('mobile.mp4', unsupported)).toEqual({
      key: 'videos.error.mp4',
      args: { name: 'mobile.mp4', code: '4', detail: 'DEMUXER_ERROR_NO_SUPPORTED_STREAMS' },
    });
    expect(playError('mobile.MP4', unsupported).key).toBe('videos.error.mp4');
    expect(playError('mobile.webm', { code: 4, detail: 'DEMUXER_ERROR_COULD_NOT_OPEN' })).toEqual({
      key: 'videos.error.play',
      args: { name: 'mobile.webm', code: '4', detail: 'DEMUXER_ERROR_COULD_NOT_OPEN' },
    });
    expect(playError('mobile.mp4', { code: 2, detail: 'PIPELINE_ERROR_NETWORK' }).key).toBe('videos.error.play');
    expect(playError('mobile.webm', { code: 0, detail: 'MEDIA_ERR_UNKNOWN' }).args.code).toBe('0');
  });

  it('says once that a video plays from memory, and why one is too large for it', () => {
    expect(memoryNotice('ipad-mini.webm', { code: 2, detail: 'PIPELINE_ERROR_NETWORK' })).toEqual({
      key: 'videos.fallback',
      args: { name: 'ipad-mini.webm', code: '2', detail: 'PIPELINE_ERROR_NETWORK' },
      level: 'info',
    });
    expect(tooLargeError('desktop.webm', 600 * 1024 ** 2 + 1, 512 * 1024 ** 2, { code: 2, detail: 'PIPELINE_ERROR_NETWORK' })).toEqual({
      key: 'videos.error.too_large',
      args: { name: 'desktop.webm', size: '601', limit: '512', code: '2', detail: 'PIPELINE_ERROR_NETWORK' },
    });
  });

  it('gives a video and its poster one box of the picture\'s shape, never taller than the panel allows', () => {
    expect(mediaBox({ width: 640, height: 1385 })).toEqual({ aspectRatio: '640 / 1385', maxWidth: 'calc(60vh * 640 / 1385)' });
    expect(mediaBox({ width: 1920, height: 1080 }).aspectRatio).toBe('1920 / 1080');
    for (const unknown of [undefined, { width: 0, height: 0 }, { width: 640, height: 0 }]) {
      expect(mediaBox(unknown)).toEqual({ aspectRatio: '16 / 9', maxWidth: 'calc(60vh * 16 / 9)' });
    }
  });
});

const files = (id: string, names: string[]): VideoRecording => ({
  id,
  path: `/p/${id}`,
  files: names.map((name) => ({
    name,
    breakpoint: name.replace(/\.[^.]+$/, ''),
    bytes: 1,
    path: `/p/${id}/${name}`,
    url: `rbrowser://video/${id}/${name}`,
  })),
});

describe('the views of a recording (RB-090-52)', () => {
  it('shows one view per breakpoint: the WebM played, its MP4 export beside it', () => {
    const views = viewsOf(files('r', ['desktop.webm', 'mobile.mp4', 'mobile.webm', 'tablet.mp4']));
    expect(views.map((view) => [view.breakpoint, view.play.name, view.mp4?.name])).toEqual([
      ['desktop', 'desktop.webm', undefined],
      ['mobile', 'mobile.webm', 'mobile.mp4'],
      ['tablet', 'tablet.mp4', undefined],
    ]);
  });

  it('offers MP4 for a WebM where the platform can, WebM for an old MP4, nothing once both are there', () => {
    const [desktop, mobile, tablet] = viewsOf(files('r', ['desktop.webm', 'mobile.mp4', 'mobile.webm', 'tablet.mp4']));
    if (desktop === undefined || mobile === undefined || tablet === undefined) {
      throw new Error('three views');
    }
    expect(offerFor(desktop, true)).toBe('mp4');
    expect(offerFor(desktop, false)).toBeNull();
    expect(offerFor(mobile, true)).toBeNull();
    expect(offerFor(tablet, false)).toBe('webm');
  });

  it('finds the conversion of a file and says how far it is', () => {
    const running = {
      recording: 'r',
      file: 'mobile.webm',
      target: 'mobile.mp4',
      to: 'mp4' as const,
      running: true,
      percent: 40,
    };
    const waiting = { ...running, file: 'desktop.webm', target: 'desktop.mp4', running: false, percent: undefined };
    expect(conversionOf([running, waiting], 'r', 'desktop.webm')).toBe(waiting);
    expect(conversionOf([running], 'other', 'mobile.webm')).toBeUndefined();
    expect(conversionOf(undefined, 'r', 'mobile.webm')).toBeUndefined();
    expect(conversionText(running)).toEqual({ key: 'videos.convert.percent', args: { target: 'mobile.mp4', percent: 40 } });
    expect(conversionText(waiting)).toEqual({ key: 'videos.convert.waiting', args: { target: 'desktop.mp4' } });
    expect(conversionText({ ...running, percent: undefined })).toEqual({
      key: 'videos.convert.running',
      args: { target: 'mobile.mp4' },
    });
  });
});
