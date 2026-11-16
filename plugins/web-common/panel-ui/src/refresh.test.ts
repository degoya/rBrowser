import { describe, expect, it } from 'vitest';
import { latestRefresh } from './refresh';

// Review of RB-080-84: a panel's read is never dropped while another is on its way; the newest
// state wins and an older answer is not shown.
function deferred<T>() {
  let resolve: (value: T) => void = () => undefined;
  let reject: (error: unknown) => void = () => undefined;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

describe('latestRefresh', () => {
  it('runs a request made during a read once that read is in, and shows only the newest answer', async () => {
    const calls: ReturnType<typeof deferred<string>>[] = [];
    let args = 'a';
    const asked: string[] = [];
    const shown: string[] = [];
    const reader = latestRefresh(
      () => {
        asked.push(args);
        const one = deferred<string>();
        calls.push(one);
        return one.promise;
      },
      (value) => shown.push(value),
      () => shown.push('failed'),
    );
    const first = reader.refresh();
    args = 'b';
    void reader.refresh();
    args = 'c';
    void reader.refresh();
    expect(asked).toEqual(['a']);
    calls[0]?.resolve('answer a');
    await Promise.resolve();
    await Promise.resolve();
    expect(asked).toEqual(['a', 'c']);
    calls[1]?.resolve('answer c');
    await first;
    expect(shown).toEqual(['answer c']);
  });

  it('lets a poll pass while a read is on its way, and drops a stale failure', async () => {
    const calls: ReturnType<typeof deferred<string>>[] = [];
    const shown: string[] = [];
    const reader = latestRefresh(
      () => {
        const one = deferred<string>();
        calls.push(one);
        return one.promise;
      },
      (value) => shown.push(value),
      () => shown.push('failed'),
    );
    const first = reader.refresh();
    void reader.poll();
    expect(calls).toHaveLength(1);
    void reader.refresh();
    calls[0]?.reject(new Error('old'));
    await Promise.resolve();
    await Promise.resolve();
    calls[1]?.resolve('new');
    await first;
    expect(shown).toEqual(['new']);
    expect(reader.busy()).toBe(false);
  });
});
