// The page script's life against a fake bridge: the hello's answer, the component's posts, a
// settings change and the stop.
import { beforeEach, describe, expect, it } from 'vitest';
import { fakePlugin, settle } from '../../../web-common/src/harness';
import { loadPage, pluginFile } from '../../../web-common/src/load';
import { fixtureEnv } from './fixture-env';
import { HOST_TAG } from './overlay';
import { start } from './start';

beforeEach(() => {
  loadPage(pluginFile('tests/fixtures/containers.html'));
  for (const layers of document.querySelectorAll('rbrowser-layers')) layers.remove();
});

describe('the page script', () => {
  it('asks for the state and shows the overlay when it is on', async () => {
    const plugin = fakePlugin('container-queries', () => ({ on: true, settings: { labels: true, color: '#0d9488' } }));
    const overlay = start(plugin, document, window, fixtureEnv);
    await settle();
    expect(plugin.sent).toEqual([{ type: 'hello' }]);
    expect(overlay.shown).toBe(true);
    expect(overlay.containers).toHaveLength(3);
    expect(document.querySelector(HOST_TAG)?.getAttribute('data-rbrowser-plugin')).toBe('container-queries');
    plugin.stop();
    expect(document.querySelector(HOST_TAG)).toBeNull();
  });

  it('follows a post over a late answer and asks again when the settings changed', async () => {
    const plugin = fakePlugin('container-queries', () => ({ on: false }));
    const overlay = start(plugin, document, window, fixtureEnv);
    plugin.post({ type: 'state', on: true, settings: {} });
    await settle();
    expect(overlay.shown).toBe(true);
    plugin.post({ type: 'other' });
    expect(overlay.shown).toBe(true);
    plugin.post({ $settings: true });
    await settle();
    expect(plugin.sent).toHaveLength(2);
    expect(overlay.shown).toBe(false);
    plugin.stop();
  });
});
