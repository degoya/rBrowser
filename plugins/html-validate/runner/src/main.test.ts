// The entry the component calls: it picks the engine, and a throw inside one is the answer's
// error with its reason, never an exception out of the view.
import { describe, expect, it } from 'vitest';
import valid from '../../tests/fixtures/valid.html?raw';
import { start } from './main';

describe('start', () => {
  it('runs the offline engine', async () => {
    const answer = await start({
      engine: 'builtin',
      source: valid,
      rules: { 'no-dup-id': 'error' },
      maxMessages: 10,
      maxText: 100,
    });
    expect(answer).toMatchObject({ engine: 'builtin', messages: [], truncated: false });
  });

  it('runs the Nu engine with the fetch it is given', async () => {
    const answer = await start(
      {
        engine: 'nu',
        endpoint: 'http://localhost:8888/?out=json',
        source: valid,
        maxMessages: 10,
        maxText: 100,
        timeoutMs: 1000,
      },
      async () => new Response('{"messages":[]}', { status: 200 }),
    );
    expect(answer).toEqual({ engine: 'nu', version: '', messages: [], truncated: false });
  });

  it('answers an unknown engine with an error', async () => {
    const answer = await start({ engine: 'other' } as never);
    expect(answer).toMatchObject({ error: 'unknown engine' });
  });

  it('answers a throw of the engine with its reason', async () => {
    const answer = await start({
      engine: 'builtin',
      source: null as unknown as string,
      rules: {},
      maxMessages: 10,
      maxText: 100,
    });
    expect(answer).toMatchObject({ engine: 'builtin' });
    expect('error' in answer && answer.error.length).toBeGreaterThan(0);
  });
});
