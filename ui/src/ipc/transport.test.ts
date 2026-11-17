import { afterEach, describe, expect, it, vi } from 'vitest';
import { EVENT_NAME, IpcErrorCode, type CoreToShell, type ShellToCore } from './protocol';
import { createCefTransport, hasCefQuery, IpcError, type CefQueryOptions, type CefWindow } from './transport';

/** A window whose `cefQuery` answers through `answer`. */
function fakeWindow(answer: (options: CefQueryOptions) => void): CefWindow & { requests: string[] } {
  const target = new EventTarget();
  const requests: string[] = [];
  return Object.assign(target, {
    requests,
    cefQuery(options: CefQueryOptions): number {
      requests.push(options.request);
      answer(options);
      return requests.length;
    },
    cefQueryCancel: vi.fn(),
  });
}

afterEach(() => {
  vi.useRealTimers();
});

describe('the cefQuery transport', () => {
  it('sends the request as JSON and parses the answer', async () => {
    const win = fakeWindow((o) => o.onSuccess('{"type":"ack"}'));
    const transport = createCefTransport(win);
    const message: ShellToCore = { type: 'navigate', url: 'http://localhost:5173/' };
    await expect(transport.send(message)).resolves.toEqual({ type: 'ack' });
    expect(JSON.parse(win.requests[0] ?? '')).toEqual(message);
  });

  it('treats an empty answer as an ack', async () => {
    const transport = createCefTransport(fakeWindow((o) => o.onSuccess('')));
    await expect(transport.send({ type: 'back' })).resolves.toEqual({ type: 'ack' });
  });

  it('rejects with the failure code and message', async () => {
    const transport = createCefTransport(
      fakeWindow((o) => o.onFailure(IpcErrorCode.NotImplemented, 'later')),
    );
    const error = await transport.send({ type: 'getAbout' }).catch((e: unknown) => e);
    expect(error).toBeInstanceOf(IpcError);
    expect((error as IpcError).code).toBe(501);
    expect((error as IpcError).message).toBe('later');
  });

  it('rejects an answer that is not a message', async () => {
    const transport = createCefTransport(fakeWindow((o) => o.onSuccess('[1,2]')));
    const error = await transport.send({ type: 'getState' }).catch((e: unknown) => e);
    expect((error as IpcError).code).toBe(IpcErrorCode.Internal);
  });

  it('times out and cancels a query that never answers', async () => {
    vi.useFakeTimers();
    const win = fakeWindow(() => undefined);
    const transport = createCefTransport(win, { timeoutMs: 50 });
    const pending = transport.send({ type: 'getState' }).catch((e: unknown) => e);
    await vi.advanceTimersByTimeAsync(60);
    const error = await pending;
    expect((error as IpcError).code).toBe(IpcErrorCode.Internal);
    expect(win.cefQueryCancel).toHaveBeenCalledWith(1);
  });

  // RB-080-90: a plugin panel's call waits for the plugin's own time, not the default.
  it('waits for one request as long as it is told to', async () => {
    vi.useFakeTimers();
    let answer: ((response: string) => void) | undefined;
    const transport = createCefTransport(fakeWindow((o) => (answer = o.onSuccess)), { timeoutMs: 50 });
    const pending = transport.send({ type: 'getState' }, { timeoutMs: 500 }).catch((e: unknown) => e);
    await vi.advanceTimersByTimeAsync(400);
    answer?.('{"type":"ack"}');
    await expect(pending).resolves.toEqual({ type: 'ack' });
  });

  it('delivers core events from the rb-ipc CustomEvent', () => {
    const win = fakeWindow(() => undefined);
    const transport = createCefTransport(win);
    const seen: CoreToShell[] = [];
    const stop = transport.subscribe((m) => seen.push(m));
    win.dispatchEvent(new CustomEvent(EVENT_NAME, { detail: { type: 'ack' } }));
    win.dispatchEvent(new CustomEvent(EVENT_NAME, { detail: 'not a message' }));
    stop();
    win.dispatchEvent(new CustomEvent(EVENT_NAME, { detail: { type: 'ack' } }));
    expect(seen).toEqual([{ type: 'ack' }]);
  });

  it('knows whether cefQuery is there', () => {
    expect(hasCefQuery(fakeWindow(() => undefined))).toBe(true);
    expect(hasCefQuery(new EventTarget())).toBe(false);
  });
});
