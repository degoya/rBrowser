import { describe, expect, it } from 'vitest';

import { ErrorCode, RpcError } from '../../rpc.ts';
import { Router } from '../../router.ts';
import { registerLighthouse } from './index.ts';
import type { Runner } from './jobs.ts';

const ctx = { signal: new AbortController().signal, progress: () => {} };

const START = {
  url: 'http://localhost:5173/',
  label: 'desktop',
  settings: {
    formFactor: 'desktop',
    screenEmulation: { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false },
    throttlingMethod: 'simulate',
    onlyCategories: ['seo'],
  },
};

/** A fake target: answers a fixed report for the page it is asked about. */
const fake: Runner = async (request) => ({
  json: JSON.stringify({ requestedUrl: request.url, configSettings: { screenEmulation: request.settings.screenEmulation } }),
  html: `<html>${request.settings.screenEmulation.width}px</html>`,
  finalUrl: request.url,
  lighthouseVersion: '13.6.0',
});

function setup(runner: Runner = fake) {
  const router = new Router();
  let next = 0;
  registerLighthouse(router, { runner, now: () => 0, newId: () => `lh-${++next}` });
  return router;
}

describe('the lighthouse methods', () => {
  it('registers exactly the methods the plugin manifest may grant', () => {
    expect(setup().methods()).toEqual([
      'lighthouse.cancel',
      'lighthouse.poll',
      'lighthouse.release',
      'lighthouse.report',
      'lighthouse.start',
      'lighthouse.status',
    ]);
  });

  it('starts, polls, hands out both reports and releases a job', async () => {
    const router = setup();
    const { job } = (await router.call('lighthouse.start', START, ctx)) as { job: string };
    const poll = await router.call('lighthouse.poll', { job, waitMs: 1000 }, ctx);
    expect(poll).toMatchObject({ job, state: 'done', label: 'desktop', result: { lighthouseVersion: '13.6.0' } });
    const html = (await router.call('lighthouse.report', { job, format: 'html' }, ctx)) as { text: string; next: number | null };
    expect(html).toMatchObject({ text: '<html>1440px</html>', next: null });
    const json = (await router.call('lighthouse.report', { job, format: 'json' }, ctx)) as { text: string };
    expect(JSON.parse(json.text)).toMatchObject({ configSettings: { screenEmulation: { width: 1440, mobile: false } } });
    expect(await router.call('lighthouse.release', { job }, ctx)).toEqual({ released: true });
    await expect(router.call('lighthouse.poll', { job }, ctx)).rejects.toBeInstanceOf(RpcError);
  });

  it('refuses a start that does not read', async () => {
    const error = await setup()
      .call('lighthouse.start', { ...START, url: 'file:///C:/secret.txt' }, ctx)
      .catch((e: unknown) => e);
    expect((error as RpcError).code).toBe(ErrorCode.InvalidParams);
  });

  it('reports the state and cancels', async () => {
    const router = setup(() => new Promise(() => {}));
    await router.call('lighthouse.start', START, ctx);
    await router.call('lighthouse.start', START, ctx);
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(await router.call('lighthouse.status', {}, ctx)).toEqual({
      running: { job: 'lh-1', label: 'desktop', url: 'http://localhost:5173/' },
      queued: 1,
      busy: true,
    });
    expect(await router.call('lighthouse.cancel', {}, ctx)).toEqual({ cancelled: 2 });
  });

  it("keeps each caller to its own jobs: cancel, status, poll, report, release (review L4 of Charge 7)", async () => {
    const router = setup(() => new Promise(() => {}));
    const mine = { ...START, $caller: 'lighthouse' };
    const { job } = (await router.call('lighthouse.start', mine, ctx)) as { job: string };
    await new Promise((resolve) => setTimeout(resolve, 0));
    const other = { $caller: 'other-plugin' };
    expect(await router.call('lighthouse.status', other, ctx)).toEqual({ running: null, queued: 0, busy: true });
    expect(await router.call('lighthouse.cancel', other, ctx)).toEqual({ cancelled: 0 });
    for (const method of ['lighthouse.poll', 'lighthouse.report', 'lighthouse.release']) {
      const error = await router
        .call(method, { job, format: 'json', ...other }, ctx)
        .catch((e: unknown) => e);
      expect((error as RpcError).code, method).toBe(-32011);
    }
    expect(await router.call('lighthouse.status', { $caller: 'lighthouse' }, ctx)).toMatchObject({
      running: { job },
    });
    expect(await router.call('lighthouse.cancel', { $caller: 'lighthouse' }, ctx)).toEqual({ cancelled: 1 });
  });

  it('refuses a caller that is no plugin id', async () => {
    const error = await setup()
      .call('lighthouse.status', { $caller: 'a b' }, ctx)
      .catch((e: unknown) => e);
    expect((error as RpcError).code).toBe(ErrorCode.InvalidParams);
  });
});
