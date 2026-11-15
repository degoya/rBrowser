// Sitemap capture in the shell (RB-050-05): the last run the core's state carries, its progress
// text, and the commands the toolbar indicator and the sitemap bar run. The capture itself runs
// in the core, in hidden views; the shell only shows it and starts, cancels or resumes it.

import type { ShellToCore, SitemapStatus, StateSnapshot, ViewId } from '../ipc/protocol';

/** Shows the sitemap bar (`Ctrl+Shift+Space P G`), sent back as `uiCommand`. */
export const PANEL_COMMAND = 'panels.sitemap';
/** Starts (or, with `resume`, continues) a capture. */
export const CAPTURE_COMMAND = 'sitemap.capture';
/** Cancels the capture in progress. */
export const CANCEL_COMMAND = 'sitemap.cancel';

/** The window's last sitemap capture; `null` before the first. */
export function sitemapOf(state: StateSnapshot | null): SitemapStatus | null {
  return state?.sitemap ?? null;
}

/** Whether `status` is a run in progress. */
export function isRunning(status: SitemapStatus | null): boolean {
  return status?.state === 'running';
}

/** The i18n key and arguments of the time left; `null` while unknown. */
export function timeLeft(
  seconds: number | undefined,
): { key: string; args: Record<string, number> } | null {
  if (seconds === undefined) {
    return null;
  }
  if (seconds < 120) {
    return { key: 'shell.sitemap.left_seconds', args: { seconds } };
  }
  return { key: 'shell.sitemap.left_minutes', args: { minutes: Math.round(seconds / 60) } };
}

/** The status line's key per state. */
const STATE_KEYS: Record<SitemapStatus['state'], string> = {
  running: 'shell.sitemap.state.running',
  cancelled: 'shell.sitemap.state.cancelled',
  finished: 'shell.sitemap.state.finished',
};

/** The i18n key of the bar's status line. */
export function stateKey(status: SitemapStatus | null): string {
  return status === null ? 'shell.sitemap.state.none' : STATE_KEYS[status.state];
}

/** `sitemap.capture` with the project's defaults (its start URL, every breakpoint). */
export function captureMessage(): ShellToCore {
  return { type: 'runCommand', id: CAPTURE_COMMAND, args: {} };
}

/** `sitemap.capture` continuing run `runId`. */
export function resumeMessage(runId: string): ShellToCore {
  return { type: 'runCommand', id: CAPTURE_COMMAND, args: { resume: runId } };
}

/** `sitemap.cancel` of run `runId`. */
export function cancelMessage(runId: string): ShellToCore {
  return { type: 'runCommand', id: CANCEL_COMMAND, args: { runId } };
}

/** The `file:` URL of a local path: a Windows drive path, a UNC path (`\\wsl.localhost\...`)
 * or a POSIX path; each segment percent-encoded. */
export function fileUrl(path: string): string {
  const slashed = path.replace(/\\/g, '/');
  const encode = (rest: string) => rest.split('/').map(encodeURIComponent).join('/');
  if (slashed.startsWith('//')) {
    return `file:${'//'}${encode(slashed.slice(2))}`;
  }
  const drive = /^([A-Za-z]):\/(.*)$/.exec(slashed);
  if (drive !== null) {
    return `file:///${drive[1]}:/${encode(drive[2] ?? '')}`;
  }
  return `file://${encode(slashed)}`;
}

/** Opens the gallery at `path` in `view` (the active one), through `view.navigate`. */
export function galleryMessage(path: string, view: ViewId): ShellToCore {
  return { type: 'runCommand', id: 'view.navigate', args: { url: fileUrl(path), target: view } };
}
