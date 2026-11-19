import { describe, expect, it } from 'vitest';
import { sampleState } from '../ipc/mockState';
import type { SitemapStatus } from '../ipc/protocol';
import {
  cancelMessage,
  captureMessage,
  galleryMessage,
  isRunning,
  resumeMessage,
  sitemapOf,
  startsShowing,
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

  it('opens the gallery through sitemap.gallery, never in a view (RB-090-83)', () => {
    expect(galleryMessage('r1')).toEqual({
      type: 'runCommand',
      id: 'sitemap.gallery',
      args: { runId: 'r1' },
    });
  });

  it('opens the bar by itself when a run starts or resumes', () => {
    expect(startsShowing(running, null)).toBe(true);
    // A resume is a start too.
    expect(startsShowing(running, { ...running, state: 'cancelled' })).toBe(true);
    expect(startsShowing(running, { ...running, done: 37 })).toBe(false);
    expect(startsShowing({ ...running, runId: 'next' }, running)).toBe(true);
    expect(startsShowing({ ...running, state: 'finished' }, null)).toBe(false);
    expect(startsShowing(null, running)).toBe(false);
  });
});
