import { describe, expect, it } from 'vitest';

import { fakePlugin, runClassicScripts, runPageScript, runPageScriptRoots, settle } from './harness';

describe('the fake bridge', () => {
  it('records what the script sent and answers with the reply', async () => {
    const plugin = fakePlugin('demo', (message) => ({ echo: message.type }));
    await expect(plugin.send({ type: 'ping', nested: { a: 1 } })).resolves.toEqual({ echo: 'ping' });
    expect(plugin.id).toBe('demo');
    expect(plugin.sent).toEqual([{ type: 'ping', nested: { a: 1 } }]);
  });

  it('rejects a send when the reply is an error, returned or thrown', async () => {
    const returned = fakePlugin('demo', () => new Error('no'));
    await expect(returned.send({})).rejects.toThrow('no');
    const thrown = fakePlugin('demo', () => {
      throw new Error('boom');
    });
    await expect(thrown.send({})).rejects.toThrow('boom');
  });

  it('delivers posted messages and the stop to the listeners', () => {
    const plugin = fakePlugin('demo', () => null);
    const heard: unknown[] = [];
    let stopped = 0;
    plugin.on((message) => heard.push(message));
    plugin.onStop(() => {
      stopped += 1;
    });
    plugin.post({ type: 'state' });
    plugin.stop();
    expect(heard).toEqual([{ type: 'state' }]);
    expect(stopped).toBe(1);
  });
});

describe('running scripts', () => {
  it('runs a page script in strict mode with rbPlugin and hands over its shadow root', () => {
    const plugin = fakePlugin('demo', () => null);
    const source = `const host = document.createElement('div');
      const root = host.attachShadow({ mode: 'closed' });
      root.innerHTML = '<b>' + rbPlugin.id + '</b>';
      document.body.append(host);`;
    const root = runPageScript(source, plugin);
    expect(root.querySelector('b')?.textContent).toBe('demo');
    expect(HTMLElement.prototype.attachShadow.name).toBe('attachShadow');
  });

  it('refuses a page script that made no or several roots, and lists all roots on request', () => {
    const plugin = fakePlugin('demo', () => null);
    expect(() => runPageScript('', plugin)).toThrow(/0 shadow roots/);
    const two = `for (const n of [1, 2]) document.createElement('div').attachShadow({ mode: 'closed' });`;
    expect(() => runPageScript(two, plugin)).toThrow(/2 shadow roots/);
    expect(runPageScriptRoots(two, plugin)).toHaveLength(2);
    expect(() => runPageScript('undefinedVariable = 1', plugin)).toThrow(ReferenceError);
  });

  it('runs classic scripts in order in one scope and takes their listeners away', () => {
    const heard: number[] = [];
    (globalThis as Record<string, unknown>).hear = (n: number) => heard.push(n);
    // The second script sees the first's `var`, as a page's scripts do.
    const remove = runClassicScripts(['var Shared = { n: 1 };', "Shared.n += 1; window.addEventListener('message', () => hear(Shared.n));"]);
    window.dispatchEvent(new Event('message'));
    expect(heard).toEqual([2]);
    remove();
    window.dispatchEvent(new Event('message'));
    expect(heard).toEqual([2]);
  });

  it('settles the asked number of microtask rounds', async () => {
    let done = false;
    void Promise.resolve().then(() => Promise.resolve()).then(() => {
      done = true;
    });
    await settle(1);
    expect(done).toBe(false);
    await settle(3);
    expect(done).toBe(true);
  });
});
