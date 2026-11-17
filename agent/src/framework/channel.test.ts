import { beforeEach, describe, expect, it } from 'vitest';
import { devtoolsInEvent, devtoolsOutEvent } from '../protocol';
import { loadPage } from '../test-page';
import { openChannel } from './channel';

beforeEach(() => {
  loadPage('<!doctype html><html><head></head><body></body></html>');
});

describe('openChannel', () => {
  it('posts envelopes as JSON text on the out event of its framework', () => {
    const details: unknown[] = [];
    document.addEventListener(devtoolsOutEvent('react'), (event) => {
      details.push((event as CustomEvent<unknown>).detail);
    });
    const channel = openChannel(window, 'react');
    channel.post({ t: 'm', d: { event: 'operations', payload: [1, 2] } });
    channel.post({ t: 'disconnect' });
    expect(details).toEqual([
      '{"t":"m","d":{"event":"operations","payload":[1,2]}}',
      '{"t":"disconnect"}',
    ]);
  });

  it('hands valid envelopes of the in event over, nothing else, until stopped', () => {
    const seen: unknown[] = [];
    const channel = openChannel(window, 'vue');
    const stop = channel.on((envelope) => seen.push(envelope));
    const send = (detail: unknown, name = devtoolsInEvent('vue')): void => {
      document.dispatchEvent(new CustomEvent(name, { detail }));
    };
    send('{"t":"connect"}');
    send('{"t":"m","d":[1,"x"]}');
    send('{"t":"other"}');
    send('{"t":"m"}');
    send({ t: 'connect' });
    send('nope');
    send('{"t":"connect"}', devtoolsInEvent('react'));
    send('{"t":"connect"}', devtoolsOutEvent('vue'));
    expect(seen).toEqual([{ t: 'connect' }, { t: 'm', d: [1, 'x'] }]);
    stop();
    send('{"t":"connect"}');
    expect(seen).toHaveLength(2);
  });

  it('keeps working when the page replaces JSON and CustomEvent later', () => {
    const channel = openChannel(window, 'react');
    const details: unknown[] = [];
    document.addEventListener(devtoolsOutEvent('react'), (event) => {
      details.push((event as CustomEvent<unknown>).detail);
    });
    const scope = window as unknown as Record<string, unknown>;
    const json = scope.JSON;
    scope.JSON = { stringify: () => 'broken', parse: () => 'broken' };
    try {
      channel.post({ t: 'connect' });
    } finally {
      scope.JSON = json;
    }
    expect(details).toEqual(['{"t":"connect"}']);
  });
});
