import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { Agent } from './agent';
import { followRoute } from './spa';

let pops = 0;
let hashes = 0;
const onPop = (): void => {
  pops += 1;
};
const onHash = (): void => {
  hashes += 1;
};

beforeEach(() => {
  window.history.replaceState(null, '', '/list');
  pops = 0;
  hashes = 0;
  window.addEventListener('popstate', onPop);
  window.addEventListener('hashchange', onHash);
});

afterEach(() => {
  window.removeEventListener('popstate', onPop);
  window.removeEventListener('hashchange', onHash);
});

describe('followRoute', () => {
  it('pushes the route and tells the page router with a popstate', () => {
    const origin = window.location.origin;
    expect(followRoute(window, `${origin}/about?tab=2`)).toEqual({ status: 'done' });
    expect(window.location.pathname).toBe('/about');
    expect(window.location.search).toBe('?tab=2');
    expect(pops).toBe(1);
  });

  it('changes only the hash for a fragment of the same page', () => {
    const origin = window.location.origin;
    expect(followRoute(window, `${origin}/list#part-2`)).toEqual({ status: 'done' });
    expect(window.location.pathname).toBe('/list');
    expect(window.location.hash).toBe('#part-2');
  });

  it('drops a hash with a popstate and a hashchange', () => {
    const origin = window.location.origin;
    window.history.replaceState(null, '', '/list#part-2');
    expect(followRoute(window, `${origin}/list`)).toEqual({ status: 'done' });
    expect(window.location.hash).toBe('');
    expect(window.location.href).toBe(`${origin}/list`);
    expect(pops).toBe(1);
    expect(hashes).toBe(1);
  });

  it('does nothing for the page it is on', () => {
    expect(followRoute(window, window.location.href)).toEqual({ status: 'done' });
    expect(pops).toBe(0);
  });

  it('refuses another origin and what is not a URL', () => {
    for (const url of [
      'https://elsewhere.example/about',
      'javascript:alert(1)',
      'data:text/html,x',
      'http://[bad',
    ]) {
      expect(followRoute(window, url)).toMatchObject({ status: 'error' });
    }
    expect(followRoute(window, 42)).toEqual({ status: 'error', message: 'invalid url' });
    expect(window.location.pathname).toBe('/list');
    expect(pops).toBe(0);
  });
});

describe('the navigate command', () => {
  it('runs followRoute', () => {
    const agent = new Agent({ win: window, send: () => undefined });
    const url = `${window.location.origin}/settings`;
    expect(agent.command({ cmd: 'navigate', url })).toEqual({ status: 'done' });
    expect(window.location.pathname).toBe('/settings');
    expect(agent.command({ cmd: 'navigate' })).toEqual({ status: 'error', message: 'invalid url' });
  });
});
