import { afterEach, describe, expect, it, vi } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { IpcClient } from '../ipc/client';
import type { CoreToShell, ShellToCore, VideoList, VideoRecording } from '../ipc/protocol';
import { IpcError } from '../ipc/transport';
import { mountWithShell, settle } from '../test/mountShell';
import VideosPage from './VideosPage.vue';

const NEW = '2026-11-16_14-03-07';
const EARLIER = '2026-11-16_09-00-00';
const OLD = '2026-11-15_10-30-00';

function recording(id: string, names: string[]): VideoRecording {
  return {
    id,
    path: `C:/shop/.rbrowser/out/video/${id}`,
    files: names.map((name) => ({
      name,
      breakpoint: name.replace(/\.[^.]+$/, ''),
      bytes: 2 * 1024 * 1024,
      path: `C:/shop/.rbrowser/out/video/${id}/${name}`,
      url: `rbrowser://video/${id}/${name}`,
    })),
  };
}

const LIST: VideoList = {
  project: true,
  folder: 'C:/shop/.rbrowser/out/video',
  autoplay: false,
  mp4Export: true,
  recordings: [
    recording(NEW, ['desktop.mp4', 'mobile.mp4', 'tablet.mp4']),
    recording(EARLIER, ['mobile.mp4']),
    recording(OLD, ['mobile.webm']),
  ],
};

let current: VueWrapper | null = null;

/** The person picks recording `id`. */
async function pick(wrapper: VueWrapper, id: string): Promise<void> {
  await wrapper.get(`[data-testid="recording-${id}"]`).trigger('click');
  await settle();
}
afterEach(() => {
  current?.unmount();
  current = null;
  vi.restoreAllMocks();
});

/** The page over a client whose `getVideos` answers `list` (or fails with `failure`). */
async function mounted(list: VideoList = LIST, failure?: IpcError) {
  const listeners = new Set<(message: CoreToShell) => void>();
  const subscribe = (listener: (message: CoreToShell) => void) => {
    listeners.add(listener);
    return () => listeners.delete(listener);
  };
  const asked: ShellToCore[] = [];
  let answer: VideoList = list;
  let fail = failure;
  const client = new IpcClient({
    send: async (message) => {
      asked.push(message);
      if (message.type === 'getVideos' || message.type === 'deleteVideo' || message.type === 'convertVideo') {
        if (fail !== undefined) {
          throw fail;
        }
        return { type: 'videos', ...answer };
      }
      return { type: 'ack' };
    },
    subscribe: () => () => undefined,
  });
  const copied: string[] = [];
  const clipboard = {
    writeText: async (text: string) => {
      copied.push(text);
    },
  };
  const result = await mountWithShell(VideosPage, { mount: { props: { client, subscribe, clipboard } } });
  current = result.wrapper;
  await settle();
  const push = (message: CoreToShell) => listeners.forEach((listener) => listener(message));
  return {
    wrapper: result.wrapper,
    asked,
    copied,
    push,
    answerNext: (next: VideoList) => {
      answer = next;
    },
    heal: () => {
      fail = undefined;
    },
    breakWith: (error: IpcError) => {
      fail = error;
    },
  };
}

describe('the Videos panel', () => {
  it('lists the recordings by day and loads no video before one is picked', async () => {
    const { wrapper, asked } = await mounted({ ...LIST, autoplay: true });
    expect(asked).toContainEqual({ type: 'getVideos' });
    const days = wrapper.findAll('[data-testid="videos-day"]').map((day) => day.text());
    expect(days).toEqual(['2026-11-16', '2026-11-15']);
    expect(wrapper.get(`[data-testid="recording-${NEW}"]`).text()).toContain('14:03:07');
    expect(wrapper.find('[aria-current="true"]').exists()).toBe(false);
    expect(wrapper.findAll('video')).toHaveLength(0);
    expect(wrapper.find('[data-testid="videos-choose"]').exists()).toBe(true);
  });

  it('shows a picked recording with every view side by side', async () => {
    const { wrapper } = await mounted();
    await pick(wrapper, NEW);
    expect(wrapper.get(`[data-testid="recording-${NEW}"]`).attributes('aria-current')).toBe('true');
    const videos = wrapper.findAll('video');
    expect(videos.map((video) => video.attributes('src'))).toEqual([
      `rbrowser://video/${NEW}/desktop.mp4`,
      `rbrowser://video/${NEW}/mobile.mp4`,
      `rbrowser://video/${NEW}/tablet.mp4`,
    ]);
    expect(wrapper.get('[data-testid="video-mobile.mp4"]').text()).toContain('2.0 MB');
  });

  it('switches to another recording', async () => {
    const { wrapper } = await mounted();
    await pick(wrapper, NEW);
    await wrapper.get(`[data-testid="recording-${OLD}"]`).trigger('click');
    expect(wrapper.findAll('video').map((video) => video.attributes('src'))).toEqual([
      `rbrowser://video/${OLD}/mobile.webm`,
    ]);
  });

  it('plays every view of the recording with one button', async () => {
    const play = vi.spyOn(HTMLMediaElement.prototype, 'play').mockImplementation(() => Promise.resolve());
    const { wrapper } = await mounted();
    await pick(wrapper, NEW);
    await wrapper.get('[data-testid="video-play"]').trigger('click');
    await settle();
    expect(play).toHaveBeenCalledTimes(3);
  });

  it('asks before it deletes a recording and says how many videos go', async () => {
    const { wrapper, asked, answerNext } = await mounted();
    await pick(wrapper, NEW);
    await wrapper.get('[data-testid="delete-recording"]').trigger('click');
    const question = wrapper.get('[data-testid="delete-confirm"]');
    expect(question.text()).toContain('14:03:07');
    expect(question.text()).toContain('3');
    expect(asked.some((message) => message.type === 'deleteVideo')).toBe(false);
    await wrapper.get('[data-testid="delete-cancel"]').trigger('click');
    expect(wrapper.find('[data-testid="delete-confirm"]').exists()).toBe(false);
    expect(asked.some((message) => message.type === 'deleteVideo')).toBe(false);

    answerNext({ ...LIST, recordings: LIST.recordings.slice(1) });
    await wrapper.get('[data-testid="delete-recording"]').trigger('click');
    await wrapper.get('[data-testid="delete-accept"]').trigger('click');
    await settle();
    expect(asked).toContainEqual({ type: 'deleteVideo', recording: NEW });
    expect(wrapper.find(`[data-testid="recording-${NEW}"]`).exists()).toBe(false);
    expect(wrapper.find('[aria-current="true"]').exists()).toBe(false);
    expect(wrapper.findAll('video')).toHaveLength(0);
  });

  it('asks before it deletes one view', async () => {
    const { wrapper, asked } = await mounted();
    await pick(wrapper, NEW);
    await wrapper.get('[data-testid="delete-mobile.mp4"]').trigger('click');
    expect(wrapper.get('[data-testid="delete-confirm"]').text()).toContain('mobile.mp4');
    await wrapper.get('[data-testid="delete-accept"]').trigger('click');
    await settle();
    expect(asked).toContainEqual({ type: 'deleteVideo', recording: NEW, file: 'mobile.mp4' });
  });

  it('shows a recording or a view in the folder and copies a path', async () => {
    const { wrapper, asked, copied } = await mounted();
    await pick(wrapper, NEW);
    await wrapper.get('[data-testid="reveal-recording"]').trigger('click');
    await wrapper.get('[data-testid="reveal-tablet.mp4"]').trigger('click');
    await wrapper.get('[data-testid="copy-tablet.mp4"]').trigger('click');
    await settle();
    expect(asked).toContainEqual({ type: 'revealVideo', recording: NEW });
    expect(asked).toContainEqual({ type: 'revealVideo', recording: NEW, file: 'tablet.mp4' });
    expect(copied).toEqual([`C:/shop/.rbrowser/out/video/${NEW}/tablet.mp4`]);
  });

  it('asks again when the core says the recordings changed', async () => {
    const { wrapper, push, answerNext } = await mounted();
    await pick(wrapper, NEW);
    answerNext({ ...LIST, recordings: [recording('2026-11-17_08-00-00', ['mobile.mp4']), ...LIST.recordings] });
    push({ type: 'videosChanged' });
    await settle();
    expect(wrapper.find('[data-testid="recording-2026-11-17_08-00-00"]').exists()).toBe(true);
    expect(wrapper.get(`[data-testid="recording-${NEW}"]`).attributes('aria-current')).toBe('true');
  });

  it('autoplays a pick once, not again when a deleted view brings the player back', async () => {
    const play = vi.spyOn(HTMLMediaElement.prototype, 'play').mockImplementation(() => Promise.resolve());
    const { wrapper, answerNext } = await mounted({ ...LIST, autoplay: true });
    await pick(wrapper, NEW);
    await wrapper.get('video').trigger('loadedmetadata');
    await settle();
    expect(play).toHaveBeenCalledTimes(3);
    answerNext({ ...LIST, autoplay: true, recordings: [recording(NEW, ['desktop.mp4', 'tablet.mp4'])] });
    await wrapper.get('[data-testid="delete-mobile.mp4"]').trigger('click');
    await wrapper.get('[data-testid="delete-accept"]').trigger('click');
    await settle();
    expect(wrapper.findAll('video')).toHaveLength(2);
    await wrapper.get('video').trigger('loadedmetadata');
    await settle();
    expect(play).toHaveBeenCalledTimes(3);
  });

  it('tells no project, no recordings and a failure apart', async () => {
    const none = await mounted({ project: false, autoplay: false, mp4Export: false, recordings: [] });
    expect(none.wrapper.find('[data-testid="videos-no-project"]').exists()).toBe(true);
    none.wrapper.unmount();
    const empty = await mounted({ ...LIST, recordings: [] });
    expect(empty.wrapper.find('[data-testid="videos-empty"]').exists()).toBe(true);
    expect(empty.wrapper.find('[data-testid="videos-failed"]').exists()).toBe(false);
    empty.wrapper.unmount();
    const linked = await mounted({ ...LIST, recordings: [], error: 'a link on the way' });
    expect(linked.wrapper.get('[data-testid="videos-failed"]').attributes('role')).toBe('alert');
    expect(linked.wrapper.find('[data-testid="videos-empty"]').exists()).toBe(false);
    linked.wrapper.unmount();
    const failed = await mounted(LIST, new IpcError(500, 'boom'));
    expect(failed.wrapper.get('[data-testid="videos-failed"]').attributes('role')).toBe('alert');
    expect(failed.wrapper.find('[data-testid="videos-empty"]').exists()).toBe(false);
    failed.heal();
    await failed.wrapper.get('[data-testid="videos-retry"]').trigger('click');
    await settle();
    expect(failed.wrapper.find(`[data-testid="recording-${NEW}"]`).exists()).toBe(true);
  });

  it('shows each recording by its poster, and the posters on its players', async () => {
    const posters = (entry: VideoRecording): VideoRecording => ({
      ...entry,
      files: entry.files.map((file) => ({ ...file, poster: file.url.replace(/\.[^.]+$/, '.jpg') })),
    });
    const [first, ...rest] = LIST.recordings;
    if (first === undefined) {
      throw new Error('a recording');
    }
    const { wrapper } = await mounted({ ...LIST, recordings: [posters(first), ...rest] });
    const thumb = wrapper.get(`[data-testid="recording-${NEW}"] img`);
    expect(thumb.attributes('src')).toBe(`rbrowser://video/${NEW}/desktop.jpg`);
    expect(thumb.attributes('alt')).toBe('');
    expect(wrapper.find(`[data-testid="recording-${OLD}"] img`).exists()).toBe(false);
    expect(wrapper.findAll('video')).toHaveLength(0);
    await pick(wrapper, NEW);
    const poster = (name: string) => wrapper.get(`[data-testid="video-box-${name}"] [data-testid="poster-${name}"]`);
    expect(['desktop.mp4', 'mobile.mp4', 'tablet.mp4'].map((name) => poster(name).attributes('src'))).toEqual([
      `rbrowser://video/${NEW}/desktop.jpg`,
      `rbrowser://video/${NEW}/mobile.jpg`,
      `rbrowser://video/${NEW}/tablet.jpg`,
    ]);
    // Acceptance 2026-11-16 ("small pictures in big windows"): poster and video share one box of
    // the picture's shape - the poster's until the video knows its own -, the picture fills it.
    const box = () => wrapper.get('[data-testid="video-box-mobile.mp4"]').element as HTMLElement;
    expect(box().style.aspectRatio).toBe('16 / 9');
    expect(poster('mobile.mp4').classes()).toEqual(expect.arrayContaining(['object-contain', 'size-full', 'absolute']));
    const image = poster('mobile.mp4').element as HTMLImageElement;
    Object.defineProperty(image, 'naturalWidth', { configurable: true, value: 390 });
    Object.defineProperty(image, 'naturalHeight', { configurable: true, value: 844 });
    await poster('mobile.mp4').trigger('load');
    expect(box().style.aspectRatio).toBe('390 / 844');
    expect(box().style.maxWidth).toBe('calc(60vh * 390 / 844)');
    const video = wrapper.get('[data-testid="video-box-mobile.mp4"] video');
    expect(video.attributes('poster')).toBeUndefined();
    expect(video.classes()).toEqual(expect.arrayContaining(['object-contain', 'size-full']));
    await video.trigger('loadeddata');
    expect(wrapper.find('[data-testid="poster-mobile.mp4"]').exists()).toBe(false);
  });

  it('says under a video why it does not play and reports it to the status bar once', async () => {
    const { wrapper, asked } = await mounted();
    await pick(wrapper, NEW);
    const fail = async (index: number, code: number) => {
      const video = wrapper.findAll('video')[index];
      if (video === undefined) {
        throw new Error('a video');
      }
      Object.defineProperty(video.element, 'error', { configurable: true, value: { code } });
      await video.trigger('error');
      await settle();
    };
    await fail(1, 4);
    await fail(1, 4);
    const line = wrapper.get('[data-testid="video-error-mobile.mp4"]');
    expect(line.attributes('role')).toBe('alert');
    expect(line.text()).toContain('mobile.mp4');
    expect(wrapper.find('[data-testid="video-error-desktop.mp4"]').exists()).toBe(false);
    const reports = asked.filter((message) => message.type === 'reportStatus');
    expect(reports).toEqual([
      {
        type: 'reportStatus',
        level: 'warning',
        messageKey: 'videos.error.mp4',
        args: { name: 'mobile.mp4', code: '4', detail: 'MEDIA_ERR_SRC_NOT_SUPPORTED' },
      },
    ]);
  });

  // Acceptance 2026-11-16: no WebM streamed from the route. The panel loads it whole once and
  // plays it from memory, says so with the player's error, and says why when that fails too.
  describe('a WebM the player cannot stream', () => {
    const made: string[] = [];
    const revoked: string[] = [];
    let fetched: string[] = [];
    let answer: () => Response = () => new Response(new Blob(['webm'], { type: 'video/webm' }));
    const originals = { create: URL.createObjectURL, revoke: URL.revokeObjectURL };
    const breakVideo = async (wrapper: VueWrapper, name: string, message: string) => {
      const video = wrapper.get(`[data-testid="video-box-${name}"] video`);
      Object.defineProperty(video.element, 'error', { configurable: true, value: { code: 4, message } });
      await video.trigger('error');
      await settle();
      await settle();
      return video;
    };
    afterEach(() => {
      URL.createObjectURL = originals.create;
      URL.revokeObjectURL = originals.revoke;
      vi.unstubAllGlobals();
      made.length = 0;
      revoked.length = 0;
      fetched = [];
      answer = () => new Response(new Blob(['webm'], { type: 'video/webm' }));
    });
    const stub = () => {
      vi.stubGlobal('fetch', async (url: string) => {
        fetched.push(url);
        return answer();
      });
      URL.createObjectURL = () => {
        const url = `blob:rbrowser://ui/${made.length + 1}`;
        made.push(url);
        return url;
      };
      URL.revokeObjectURL = (url: string) => {
        revoked.push(url);
      };
    };

    it('plays it from memory and says so once, with the error', async () => {
      stub();
      const { wrapper, asked } = await mounted();
      await pick(wrapper, OLD);
      const video = await breakVideo(wrapper, 'mobile.webm', 'PIPELINE_ERROR_NETWORK: stream failed');
      expect(fetched).toEqual([`rbrowser://video/${OLD}/mobile.webm`]);
      expect(video.attributes('src')).toBe('blob:rbrowser://ui/1');
      expect(wrapper.find('[data-testid="video-error-mobile.webm"]').exists()).toBe(false);
      expect(asked.filter((message) => message.type === 'reportStatus')).toEqual([
        {
          type: 'reportStatus',
          level: 'info',
          messageKey: 'videos.fallback',
          args: { name: 'mobile.webm', code: '4', detail: 'PIPELINE_ERROR_NETWORK: stream failed' },
        },
      ]);
      // Leaving the recording revokes the copy.
      await pick(wrapper, NEW);
      expect(revoked).toEqual(['blob:rbrowser://ui/1']);
    });

    it('says why when it does not play from memory either', async () => {
      stub();
      const { wrapper, asked } = await mounted();
      await pick(wrapper, OLD);
      await breakVideo(wrapper, 'mobile.webm', 'PIPELINE_ERROR_NETWORK');
      await breakVideo(wrapper, 'mobile.webm', 'DEMUXER_ERROR_COULD_NOT_OPEN: FFmpegDemuxer: open context failed');
      expect(fetched).toHaveLength(1);
      const line = wrapper.get('[data-testid="video-error-mobile.webm"]');
      expect(line.text()).toContain('DEMUXER_ERROR_COULD_NOT_OPEN');
      const reports = asked.filter((message) => message.type === 'reportStatus');
      expect(reports.map((report) => (report.type === 'reportStatus' ? report.messageKey : ''))).toEqual([
        'videos.fallback',
        'videos.error.play',
      ]);
    });

    it('says why when the whole file does not load', async () => {
      stub();
      answer = () => new Response('', { status: 404 });
      const { wrapper, asked } = await mounted();
      await pick(wrapper, OLD);
      await breakVideo(wrapper, 'mobile.webm', 'PIPELINE_ERROR_NETWORK');
      expect(wrapper.get('[data-testid="video-error-mobile.webm"]').text()).toContain('PIPELINE_ERROR_NETWORK');
      expect(made).toEqual([]);
      const keys = asked.filter((message) => message.type === 'reportStatus').map((report) => (report.type === 'reportStatus' ? report.messageKey : ''));
      expect(keys).toEqual(['videos.error.play']);
    });

    it('does not load one larger than 512 MiB', async () => {
      stub();
      const big = recording(OLD, ['mobile.webm']);
      const [only] = big.files;
      if (only === undefined) {
        throw new Error('a file');
      }
      only.bytes = 600 * 1024 * 1024;
      const { wrapper } = await mounted({ ...LIST, recordings: [big] });
      await pick(wrapper, OLD);
      await breakVideo(wrapper, 'mobile.webm', 'PIPELINE_ERROR_NETWORK');
      expect(fetched).toEqual([]);
      const line = wrapper.get('[data-testid="video-error-mobile.webm"]').text();
      expect(line).toContain('600 MB');
      expect(line).toContain('512 MB');
    });
  });

  // RB-090-52: sharing as MP4, converting old MP4s.
  it('exports a WebM as MP4 in the background and shows how far it is', async () => {
    const { wrapper, asked, answerNext } = await mounted();
    await pick(wrapper, OLD);
    expect(wrapper.find('[data-testid="mp4-unavailable"]').exists()).toBe(false);
    answerNext({
      ...LIST,
      conversions: [
        { recording: OLD, file: 'mobile.webm', target: 'mobile.mp4', to: 'mp4', running: true, percent: 40 },
      ],
    });
    await wrapper.get('[data-testid="convert-mobile.webm"]').trigger('click');
    await settle();
    expect(asked).toContainEqual({ type: 'convertVideo', recording: OLD, file: 'mobile.webm', to: 'mp4' });
    const progress = wrapper.get('[data-testid="converting-mobile.webm"]');
    expect(progress.attributes('role')).toBe('status');
    expect(progress.text()).toContain('40');
    expect(wrapper.find('[data-testid="convert-mobile.webm"]').exists()).toBe(false);
  });

  it('lists the MP4 export under its WebM, one view per breakpoint', async () => {
    const both = recording(OLD, ['mobile.mp4', 'mobile.webm']);
    const { wrapper } = await mounted({ ...LIST, recordings: [both] });
    const entry = wrapper.get(`[data-testid="recording-${OLD}"]`).text();
    expect(entry).toContain('1 view');
    expect(entry).not.toContain('views');
    await pick(wrapper, OLD);
    expect(wrapper.findAll('video').map((video) => video.attributes('src'))).toEqual([
      `rbrowser://video/${OLD}/mobile.webm`,
    ]);
    expect(wrapper.get('[data-testid="export-mobile.mp4"]').text()).toContain('mobile.mp4');
    expect(wrapper.find('[data-testid="convert-mobile.webm"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="delete-mobile.mp4"]').exists()).toBe(true);
  });

  it('offers no MP4 export where the platform has no encoder, and says why', async () => {
    const { wrapper } = await mounted({ ...LIST, mp4Export: false });
    await pick(wrapper, OLD);
    expect(wrapper.find('[data-testid="convert-mobile.webm"]').exists()).toBe(false);
    expect(wrapper.get('[data-testid="mp4-unavailable"]').text()).not.toBe('');
  });

  it('converts an old MP4 to WebM', async () => {
    const { wrapper, asked } = await mounted({ ...LIST, mp4Export: false });
    await pick(wrapper, NEW);
    expect(wrapper.find('[data-testid="mp4-unavailable"]').exists()).toBe(false);
    await wrapper.get('[data-testid="convert-tablet.mp4"]').trigger('click');
    await settle();
    expect(asked).toContainEqual({ type: 'convertVideo', recording: NEW, file: 'tablet.mp4', to: 'webm' });
  });

  it('says why a conversion did not start', async () => {
    const { wrapper, breakWith } = await mounted();
    await pick(wrapper, OLD);
    breakWith(new IpcError(403, 'mobile.mp4 is there already'));
    await wrapper.get('[data-testid="convert-mobile.webm"]').trigger('click');
    await settle();
    expect(wrapper.get('[data-testid="videos-action-error"]').text()).toContain('mobile.mp4 is there already');
  });
});
