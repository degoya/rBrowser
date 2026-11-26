// Media that does not play (RB-099-29). The standard CEF build has no proprietary codecs: a
// `<video>` with an MP4 (H.264, AAC) stays black. The agent hears the `error` events of the
// frame's `<video>`, `<audio>` and `<source>` elements in the capture phase on the window (they
// do not bubble, and a page's own capture listener on an element runs after this one), keeps a
// media element's failure only with `MediaError` code 4 (`MEDIA_ERR_SRC_NOT_SUPPORTED`) and a
// source's with its `type` and what `canPlayType` says to it, and reports each once per
// document. Whether a format is missing - and which - is the core's (`media_codec`): it knows
// the formats; the agent only says what the page and the build said. An address goes without
// its query and fragment, a data URL as its media type.

import type { MediaFailure, MediaPlays } from './protocol';
import { cssPath } from './selector';

/** The most failures one document reports. */
export const MAX_MEDIA_REPORTS = 32;
/** The longest address reported. */
const MAX_SRC = 2048;
/** The longest `MediaError.message` and `type` reported. */
const MAX_TEXT = 256;
/** `MediaError.MEDIA_ERR_SRC_NOT_SUPPORTED`. */
const SRC_NOT_SUPPORTED = 4;

/** `src` as it is reported: without query and fragment, a data URL as its media type, cut. */
export function cleanSrc(src: string): string {
  if (src.startsWith('data:')) {
    const end = src.search(/[;,]/);
    return end < 0 ? 'data:' : src.slice(0, end);
  }
  const end = src.search(/[?#]/);
  return (end < 0 ? src : src.slice(0, end)).slice(0, MAX_SRC);
}

/** What the build plays of the codecs the standard build lacks. */
function playsOf(doc: Document): MediaPlays {
  const probe = doc.createElement('video');
  const can = (type: string): boolean => probe.canPlayType(type) !== '';
  return {
    h264: can('video/mp4; codecs="avc1.42E01E"'),
    aac: can('audio/mp4; codecs="mp4a.40.2"'),
    hevc: can('video/mp4; codecs="hvc1.1.6.L93.B0"'),
  };
}

function mediaTag(element: Element | null): 'video' | 'audio' | null {
  const name = element?.tagName.toLowerCase();
  return name === 'video' || name === 'audio' ? name : null;
}

/** The failure of `target`, an element the `error` event was fired at; null when it is none. */
function failureOf(target: EventTarget | null, win: Window): Omit<MediaFailure, 'plays'> | null {
  if (!target || typeof (target as Element).tagName !== 'string') return null;
  const element = target as Element;
  const url = cleanSrc(win.location.href);
  const own = mediaTag(element);
  if (own) {
    const media = element as HTMLMediaElement;
    const error = media.error;
    if (!error || error.code !== SRC_NOT_SUPPORTED) return null;
    const src = cleanSrc(media.currentSrc || media.src);
    if (!src) return null;
    return {
      url,
      tag: own,
      code: error.code,
      ...(error.message ? { message: error.message.slice(0, MAX_TEXT) } : {}),
      src,
      path: cssPath(element),
    };
  }
  if (element.tagName.toLowerCase() !== 'source') return null;
  const parent = element.parentElement;
  if (!mediaTag(parent)) return null;
  const source = element as HTMLSourceElement;
  const src = cleanSrc(source.src);
  if (!src) return null;
  const type = source.type.trim().slice(0, MAX_TEXT);
  return {
    url,
    tag: 'source',
    src,
    ...(type ? { type, canPlay: (parent as HTMLMediaElement).canPlayType(type) } : {}),
    path: cssPath(element),
  };
}

/** Calls `found` with every media failure in `win`'s document, each once and at most
 *  [`MAX_MEDIA_REPORTS`]. Returns the function that stops listening. */
export function watchMedia(win: Window, found: (failure: MediaFailure) => void): () => void {
  const reported = new Set<string>();
  let plays: MediaPlays | null = null;
  const listener = (event: Event): void => {
    if (reported.size >= MAX_MEDIA_REPORTS) return;
    let failure: Omit<MediaFailure, 'plays'> | null;
    try {
      failure = failureOf(event.target, win);
    } catch {
      return;
    }
    if (!failure) return;
    const key = `${failure.tag}\n${failure.src}\n${failure.type ?? ''}`;
    if (reported.has(key)) return;
    reported.add(key);
    plays ??= playsOf(win.document);
    found({ ...failure, plays });
  };
  win.addEventListener('error', listener, true);
  return () => win.removeEventListener('error', listener, true);
}
