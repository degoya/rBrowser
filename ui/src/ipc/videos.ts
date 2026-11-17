// The recordings as the Videos panel shows them (RB-110-12), mirror of rb-ipc's `VideoList`;
// kept beside `protocol.ts`, which re-exports it. `getVideos`, `deleteVideo` and `convertVideo`
// are answered with `{ type: 'videos', ...list }`; `videosChanged` tells the panel to ask again.
// The MP4 exports and WebM conversions waiting and running come with the list (RB-090-52).

/** One recorded view. */
export interface VideoFile {
  /** `mobile.mp4`. */
  name: string;
  /** The breakpoint (or size) it shows: the name without its extension. */
  breakpoint: string;
  bytes: number;
  /** The file's path, for "Copy path". */
  path: string;
  /** Where the panel plays it: `rbrowser://video/<id>/<name>`. */
  url: string;
  /** Its poster (the first frame, a JPEG) on the same route; absent when there is none. */
  poster?: string;
}

/** One recording: a folder with one file per view. */
export interface VideoRecording {
  /** The folder name, `2026-11-16_14-03-07`. */
  id: string;
  path: string;
  files: VideoFile[];
}

/** The recordings of the open project, newest first. */
export interface VideoList {
  /** A project is open: recordings need one. */
  project: boolean;
  /** `<root>/.rbrowser/out/video` while a project is open. */
  folder?: string;
  /** `capture.video.autoplay`. */
  autoplay: boolean;
  /** Why the folder could not be read; the list is empty then. */
  error?: string;
  recordings: VideoRecording[];
  /** A WebM can be exported as MP4 here: Windows and macOS have an H.264 encoder in the LGPL
   * ffmpeg, Linux has none (RB-090-52). */
  mp4Export: boolean;
  /** The exports and conversions waiting and running, the running one first; absent: none. */
  conversions?: VideoConversion[];
}

/** What a video is made into: an MP4 of a WebM to share, a WebM of an old MP4 to play here. */
export type VideoTarget = 'mp4' | 'webm';

/** One export or conversion, waiting or running. */
export interface VideoConversion {
  recording: string;
  /** The video it is made from, `mobile.webm`. */
  file: string;
  /** The file it makes beside it, `mobile.mp4`. */
  target: string;
  to: VideoTarget;
  /** It runs now; else it waits for the one before. */
  running: boolean;
  /** 0 to 100, once ffmpeg said the video's length. */
  percent?: number;
}

/** The panel's requests. */
export type VideoRequest =
  /** Answered with `videos`. */
  | { type: 'getVideos' }
  /** The person confirmed it in the panel; answered with `videos`. Never MCP. */
  | { type: 'deleteVideo'; recording: string; file?: string }
  /** Shows the recording or its file in the file manager; `ack`. */
  | { type: 'revealVideo'; recording: string; file?: string }
  /** Queues making `file` into `to` beside it in the background; answered with `videos`. */
  | { type: 'convertVideo'; recording: string; file: string; to: VideoTarget };

/** The answers and events. */
export type VideoEvent = ({ type: 'videos' } & VideoList) | { type: 'videosChanged' };
