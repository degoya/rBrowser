import { describe, expect, it } from 'vitest';
import { sampleState } from '../ipc/mockState';
import type { SitemapStatus } from '../ipc/protocol';
import {
  cancelMessage,
  captureMessage,
  fileUrl,
  galleryMessage,
  isRunning,
  resumeMessage,
  sitemapOf,
  stateKey,
  timeLeft,
} from './sitemap';

const running: SitemapStatus = {
  runId: '2026-11-11_10-00-00',
  state: 'running',
  total: 76,
  done: 38,
  failed: 1,
  etaSeconds: 300,
  folder: 'C:\\work\\shop\\.rbrowser\\out\\sitemap-capture\\2026-11-11_10-00-00',
};

describe('sitemap capture in the shell', () => {
  it('reads the last run from the snapshot', () => {
    const state = sampleState();
    expect(sitemapOf(null)).toBeNull();
    expect(sitemapOf(state)).toBeNull();
    state.sitemap = running;
    expect(sitemapOf(state)).toEqual(running);
    expect(isRunning(running)).toBe(true);
    expect(isRunning({ ...running, state: 'cancelled' })).toBe(false);
  });

  it('says the time left in seconds, then in minutes', () => {
    expect(timeLeft(undefined)).toBeNull();
    expect(timeLeft(40)).toEqual({ key: 'shell.sitemap.left_seconds', args: { seconds: 40 } });
    expect(timeLeft(300)).toEqual({ key: 'shell.sitemap.left_minutes', args: { minutes: 5 } });
    expect(stateKey(null)).toBe('shell.sitemap.state.none');
    expect(stateKey({ ...running, state: 'finished' })).toBe('shell.sitemap.state.finished');
  });

  it('builds the commands of the bar', () => {
    expect(captureMessage()).toEqual({ type: 'runCommand', id: 'sitemap.capture', args: {} });
    expect(resumeMessage('r1')).toEqual({
      type: 'runCommand',
      id: 'sitemap.capture',
      args: { resume: 'r1' },
    });
    expect(cancelMessage('r1')).toEqual({
      type: 'runCommand',
      id: 'sitemap.cancel',
      args: { runId: 'r1' },
    });
  });

  it('opens the gallery as a file URL in the active view', () => {
    expect(fileUrl('C:\\work\\my shop\\index.html')).toBe('file:///C:/work/my%20shop/index.html');
    expect(fileUrl('\\\\wsl.localhost\\Ubuntu\\home\\a\\index.html')).toBe(
      'file://wsl.localhost/Ubuntu/home/a/index.html',
    );
    expect(fileUrl('/home/a/#1/index.html')).toBe('file:///home/a/%231/index.html');
    expect(galleryMessage('C:\\g\\index.html', 2)).toEqual({
      type: 'runCommand',
      id: 'view.navigate',
      args: { url: 'file:///C:/g/index.html', target: 2 },
    });
  });
});
