import { afterEach, describe, expect, it } from 'vitest';
import { HOST_TAG } from './constants';
import { start } from './page';
import { Simulator } from './simulator';
import type { RbPlugin } from './types';

const DIMMED = { filter: [{ op: 'brightness', amount: 0.64 }], overlay: null };
const TUNNEL = { filter: [], overlay: { kind: 'tunnel', clear: 0.24, fade: 0.25, opacity: 0.95 } };

function bridge(answer: Promise<unknown>) {
  const listeners: Array<(message: unknown) => void> = [];
  const stops: Array<() => void> = [];
  const sent: unknown[] = [];
  const fake: RbPlugin = {
    id: 'simulators',
    view: 2,
    send: (message) => {
      sent.push(message);
      return answer;
    },
    emit: () => undefined,
    on: (listener) => {
      listeners.push(listener);
      return () => listeners.splice(listeners.indexOf(listener), 1);
    },
    onStop: (listener) => {
      stops.push(listener);
      return () => undefined;
    },
  };
  return {
    fake,
    sent,
    post: (message: unknown) => listeners.forEach((listener) => listener(message)),
    stop: () => stops.forEach((listener) => listener()),
    listeners,
  };
}

const filter = () => document.documentElement.style.getPropertyValue('filter');
const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

const running: Array<() => void> = [];
const run = (fake: RbPlugin): void => {
  running.push(start(fake, new Simulator(document)));
};

afterEach(() => {
  running.splice(0).forEach((stop) => stop());
  document.querySelector(HOST_TAG)?.remove();
  document.documentElement.removeAttribute('style');
});

describe('the page script', () => {
  it('asks for its simulation when the document starts and draws it', async () => {
    const page = bridge(Promise.resolve({ spec: DIMMED }));
    run(page.fake);
    expect(page.sent).toEqual([{ op: 'init' }]);
    await flush();
    expect(filter()).toBe('brightness(0.64)');
  });

  it('draws nothing when Chromium renders the simulation itself', async () => {
    const page = bridge(Promise.resolve({ spec: null }));
    run(page.fake);
    await flush();
    expect(filter()).toBe('');
    expect(document.querySelector(HOST_TAG)).toBeNull();
  });

  it('follows the posts, and a post beats a late answer', async () => {
    let answer: (value: unknown) => void = () => undefined;
    const page = bridge(new Promise((resolve) => (answer = resolve)));
    run(page.fake);
    page.post({ type: 'apply', spec: TUNNEL });
    answer({ spec: DIMMED });
    await flush();
    expect(filter()).toBe('');
    expect(document.querySelectorAll(HOST_TAG)).toHaveLength(1);
    page.post({ type: 'apply', spec: null });
    expect(document.querySelector(HOST_TAG)).toBeNull();
    page.post({ type: 'other', spec: DIMMED });
    expect(filter()).toBe('');
  });

  it('removes every trace when it is switched off', async () => {
    const page = bridge(Promise.resolve({ spec: { ...DIMMED, overlay: { kind: 'glare', opacity: 0.3 } } }));
    run(page.fake);
    await flush();
    expect(filter()).not.toBe('');
    page.stop();
    expect(filter()).toBe('');
    expect(document.querySelector(HOST_TAG)).toBeNull();
    expect(page.listeners).toHaveLength(0);
  });

  it('survives a failed request', async () => {
    const page = bridge(Promise.reject(new Error('plugins.error.timed_out')));
    const warnings: unknown[] = [];
    const original = console.warn;
    console.warn = (...args: unknown[]) => warnings.push(args);
    run(page.fake);
    await flush();
    console.warn = original;
    expect(warnings).toHaveLength(1);
    expect(filter()).toBe('');
  });
});
