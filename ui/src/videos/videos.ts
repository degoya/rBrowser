// The Videos panel's words and numbers (RB-110-12): the recordings grouped by day (their ids are
// `<date>_<time>`, newest first from the core), a recording named by its time of day, sizes and
// durations as people read them, the speeds offered, the deletion question - which says how
// many files go -, a recording's poster and why a video does not play. A recording's views
// (RB-090-52): one per breakpoint, the WebM played and its MP4 export beside it, an old MP4
// played (and offered for conversion) where there is no WebM.

import type { VideoConversion, VideoFile, VideoRecording } from '../ipc/protocol';
import type { MediaFailure } from './memory';

/** A message key with its arguments, for `t()`. */
export interface Text {
  key: string;
  args: Record<string, string | number>;
}

/** The speeds the player offers. */
export const SPEEDS: readonly number[] = [0.25, 0.5, 1, 1.5, 2];

const ID = /^(\d{4}-\d{2}-\d{2})_(\d{2})-(\d{2})-(\d{2})$/;

/** The recordings of one day. */
export interface DayGroup {
  day: string;
  recordings: VideoRecording[];
}

/** `recordings` by day, in their order; an id of another form is a group of its own. */
export function groupByDay(recordings: VideoRecording[]): DayGroup[] {
  const groups: DayGroup[] = [];
  for (const recording of recordings) {
    const day = ID.exec(recording.id)?.[1] ?? recording.id;
    const last = groups.at(-1);
    if (last?.day === day) {
      last.recordings.push(recording);
    } else {
      groups.push({ day, recordings: [recording] });
    }
  }
  return groups;
}

/** `14:03:07` for `2026-11-16_14-03-07`; another id as it is. */
export function recordingTime(id: string): string {
  const match = ID.exec(id);
  return match === null ? id : `${match[2]}:${match[3]}:${match[4]}`;
}

/** A size in bytes, KB, MB or GB. */
export function sizeText(bytes: number): Text {
  const units: [string, number][] = [
    ['videos.size.gb', 1024 ** 3],
    ['videos.size.mb', 1024 ** 2],
    ['videos.size.kb', 1024],
  ];
  for (const [key, unit] of units) {
    if (bytes >= unit) {
      return { key, args: { size: (bytes / unit).toFixed(1) } };
    }
  }
  return { key: 'videos.size.bytes', args: { size: bytes } };
}

/** `m:ss` or `h:mm:ss`; empty while the duration is not known. */
export function durationText(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) {
    return '';
  }
  const whole = Math.floor(seconds);
  const hours = Math.floor(whole / 3600);
  const minutes = Math.floor((whole % 3600) / 60);
  const rest = String(whole % 60).padStart(2, '0');
  return hours > 0 ? `${hours}:${String(minutes).padStart(2, '0')}:${rest}` : `${minutes}:${rest}`;
}

/** The question before a deletion: the whole recording (`file` null) with its file count, or one
 * of its files with the count of those that stay. */
export function deleteQuestion(recording: VideoRecording, file: string | null): Text {
  const count = recording.files.length;
  return file === null
    ? { key: 'videos.delete.recording', args: { time: recordingTime(recording.id), count } }
    : { key: 'videos.delete.file', args: { name: file, count: count - 1 } };
}

/** The poster the list shows for a recording: its first view's that has one. */
export function recordingPoster(recording: VideoRecording): string | undefined {
  return recording.files.find((file) => file.poster !== undefined)?.poster;
}

/** One recorded view: the video the player shows and, beside a WebM, its MP4 export. */
export interface RecordedView {
  breakpoint: string;
  /** The WebM; an old MP4 where there is none. */
  play: VideoFile;
  /** The MP4 exported from the WebM. */
  mp4?: VideoFile;
}

const isMp4 = (name: string): boolean => /\.mp4$/i.test(name);

/** The views of `recording`, one per breakpoint, in the order of its files. */
export function viewsOf(recording: VideoRecording): RecordedView[] {
  const views = new Map<string, RecordedView>();
  for (const file of recording.files) {
    const view = views.get(file.breakpoint);
    if (view === undefined) {
      views.set(file.breakpoint, { breakpoint: file.breakpoint, play: file });
    } else if (isMp4(file.name) && !isMp4(view.play.name)) {
      view.mp4 = file;
    } else if (!isMp4(file.name) && isMp4(view.play.name)) {
      views.set(file.breakpoint, { breakpoint: file.breakpoint, play: file, mp4: view.play });
    }
  }
  return [...views.values()];
}

/** What a view offers to make: an MP4 of its WebM (where the platform can), a WebM of its old
 * MP4; nothing once it has both. */
export function offerFor(view: RecordedView, mp4Export: boolean): 'mp4' | 'webm' | null {
  if (isMp4(view.play.name)) {
    return 'webm';
  }
  return view.mp4 === undefined && mp4Export ? 'mp4' : null;
}

/** The export or conversion of `file` in `recording`, waiting or running. */
export function conversionOf(
  conversions: readonly VideoConversion[] | undefined,
  recording: string,
  file: string,
): VideoConversion | undefined {
  return conversions?.find((entry) => entry.recording === recording && entry.file === file);
}

/** How far an export or conversion is, as the view's line says it. */
export function conversionText(conversion: VideoConversion): Text {
  if (!conversion.running) {
    return { key: 'videos.convert.waiting', args: { target: conversion.target } };
  }
  return conversion.percent === undefined
    ? { key: 'videos.convert.running', args: { target: conversion.target } }
    : { key: 'videos.convert.percent', args: { target: conversion.target, percent: conversion.percent } };
}

/** `MediaError.MEDIA_ERR_SRC_NOT_SUPPORTED`: the browser cannot decode the format. */
const SRC_NOT_SUPPORTED = 4;

/** Why video `name` does not play, as the line under it and the status bar say it, with
 * Chromium's text (`detail`); the `MediaError` code rides in the arguments, and both reach
 * rBrowser's log (`video::diagnose`). An
 * MP4 the browser cannot decode is rBrowser's Chromium without an H.264 decoder - an old
 * recording: the text points to "Convert to WebM" under it (RB-090-52). */
export function playError(name: string, failure: MediaFailure): Text & { settingsKey?: string } {
  const args = { name, code: String(failure.code), detail: failure.detail };
  if (failure.code === SRC_NOT_SUPPORTED && isMp4(name)) {
    return { key: 'videos.error.mp4', args };
  }
  return { key: 'videos.error.play', args };
}

/** A video that did not stream is loaded whole to play from memory (`memory.ts`): the status
 * bar and the log say so once, with the player's error, at `info`. */
export function memoryNotice(name: string, failure: MediaFailure): Text & { level: 'info' } {
  return { key: 'videos.fallback', args: { name, code: String(failure.code), detail: failure.detail }, level: 'info' };
}

/** A video that did not stream and is too large to play from memory: its size and the limit
 * per recording, in MB; the player's error rides along for the log. */
export function tooLargeError(name: string, bytes: number, limit: number, failure: MediaFailure): Text {
  const mb = (value: number): string => String(Math.ceil(value / 1024 ** 2));
  return {
    key: 'videos.error.too_large',
    args: { name, size: mb(bytes), limit: mb(limit), code: String(failure.code), detail: failure.detail },
  };
}

/** The box a video and its poster share: the picture's own aspect ratio (16:9 until one is
 * known) and never taller than 60 % of the panel, so the picture fills it - no small picture in
 * a big box. */
export function mediaBox(size?: { width: number; height: number }): { aspectRatio: string; maxWidth: string } {
  const known = size !== undefined && size.width > 0 && size.height > 0;
  const width = known ? size.width : 16;
  const height = known ? size.height : 9;
  return { aspectRatio: `${width} / ${height}`, maxWidth: `calc(60vh * ${width} / ${height})` };
}
