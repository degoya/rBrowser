// The Videos panel's channel on the shell's IPC (RB-110-12): `getVideos`, `deleteVideo` (sent
// only after the person confirmed it in the panel), `revealVideo` and `convertVideo` (an MP4
// export or a WebM conversion in the background, RB-090-52) go out with the client, and the
// core's `videosChanged` (a recording saved, the retention ran, a conversion moved on) asks for
// the list again.

import type { IpcClient } from '../ipc/client';
import type { CoreToShell, VideoList, VideoTarget } from '../ipc/protocol';
import type { Subscribe } from '../whichkey/whichkey';

/** The command that shows the panel (`Ctrl+Shift+Space C T`). */
export const PANEL_COMMAND = 'capture.video.panel';

export interface VideosChannel {
  list(): Promise<VideoList>;
  remove(recording: string, file: string | null): Promise<VideoList>;
  reveal(recording: string, file: string | null): Promise<void>;
  /** Queues making `file` into `to` beside it; answers the list with the job in it. */
  convert(recording: string, file: string, to: VideoTarget): Promise<VideoList>;
  /** Calls `listener` when the recordings changed; returns the function that stops. */
  onChanged(listener: () => void): () => void;
}

function listOf(answer: Extract<CoreToShell, { type: 'videos' }>): VideoList {
  const { type: _type, ...list } = answer;
  return list;
}

export function videosChannel(client: IpcClient, subscribe: Subscribe): VideosChannel {
  return {
    async list() {
      return listOf(await client.expect({ type: 'getVideos' }, 'videos'));
    },
    async remove(recording, file) {
      const message = file === null ? { recording } : { recording, file };
      return listOf(await client.expect({ type: 'deleteVideo', ...message }, 'videos'));
    },
    async convert(recording, file, to) {
      return listOf(await client.expect({ type: 'convertVideo', recording, file, to }, 'videos'));
    },
    async reveal(recording, file) {
      await client.request(file === null ? { type: 'revealVideo', recording } : { type: 'revealVideo', recording, file });
    },
    onChanged(listener) {
      return subscribe((message: CoreToShell) => {
        if (message.type === 'videosChanged') {
          listener();
        }
      });
    },
  };
}
