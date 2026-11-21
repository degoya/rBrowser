import { describe, expect, it } from 'vitest';
import type { PortalPresentation } from '../ipc/protocol';
import {
  isPresenting,
  linkSecondsLeft,
  newLinkMessage,
  startMessage,
  stopMessage,
} from './presentation';

const LIVE: PortalPresentation = {
  enabled: true,
  consented: true,
  phase: 'live',
  address: 'https://quick-brown-fox.trycloudflare.com',
  viewers: 1,
  maxViewers: 3,
  link: {
    url: 'https://quick-brown-fox.trycloudflare.com/__rbrowser/join#t=00',
    size: 1,
    rows: ['1'],
    expiresAtMs: 60_000,
  },
};

describe('the presentation over the internet (RB-096-05)', () => {
  it('runs while it starts or is live', () => {
    expect(isPresenting(LIVE)).toBe(true);
    expect(isPresenting({ ...LIVE, phase: 'starting' })).toBe(true);
    expect(isPresenting({ ...LIVE, phase: 'downloading' })).toBe(false);
    expect(isPresenting({ ...LIVE, phase: 'off' })).toBe(false);
    expect(isPresenting(undefined)).toBe(false);
  });

  it('starts with the consent and the download only when the person gives them', () => {
    expect(startMessage({ consent: false, download: false })).toEqual({
      type: 'runCommand',
      id: 'portal.present.start',
      args: {},
    });
    expect(startMessage({ consent: true, download: true })).toEqual({
      type: 'runCommand',
      id: 'portal.present.start',
      args: { consent: true, download: true },
    });
    expect(newLinkMessage()).toEqual({ type: 'runCommand', id: 'portal.present.new_link' });
    expect(stopMessage()).toEqual({ type: 'runCommand', id: 'portal.present.stop' });
  });

  it('counts the link down and has none without one', () => {
    expect(linkSecondsLeft(LIVE, 0)).toBe(60);
    expect(linkSecondsLeft(LIVE, 61_000)).toBe(0);
    expect(linkSecondsLeft({ ...LIVE, link: undefined }, 0)).toBe(0);
    expect(linkSecondsLeft(undefined, 0)).toBe(0);
  });
});
