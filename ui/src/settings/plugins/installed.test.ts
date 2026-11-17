import { describe, expect, it } from 'vitest';
import { commandQuery, parseInstalled, rowName, switchable } from './installed';
import { restartKey } from './labels';
import { SAMPLE_INSTALLED } from './sample';

describe('parseInstalled', () => {
  it('reads the rows, the folders and the loading flag', () => {
    const list = parseInstalled(SAMPLE_INSTALLED);
    expect(list.loading).toBe(false);
    expect(list.plugins.map(rowName)).toEqual(['seo', 'overlays', 'snippets', 'broken-2.0.0.rbplugin', 'acme']);
    expect(list.plugins[0]).toEqual({
      id: 'seo',
      file: '',
      verified: true,
      name: 'SEO',
      version: '0.1.0',
      description: '<b>Checks</b> a page for the SEO essentials.',
      source: 'bundled',
      signature: { state: 'signed', keyId: 'rbrowser-plugin-v1' },
      state: 'active',
      commands: 3,
    });
    expect(list.plugins[3]?.error?.code).toBe('plugins.error.signature_invalid');
    expect(Object.keys(list.folders)).toEqual(['bundled', 'user']);
    expect(list.missing).toEqual({});
  });

  it('gives a refused package no id, whatever the row says', () => {
    const [refused] = parseInstalled({
      plugins: [
        {
          id: 'seo',
          name: 'SEO',
          version: '9.9.9',
          description: 'text',
          file: 'evil.rbplugin',
          verified: false,
          declared: { name: 'SEO', version: '9.9.9', claimsIdOf: 'SEO' },
          source: 'project',
          state: 'error',
          signature: { state: 'signed', keyId: 'k' },
          commands: 4,
        },
      ],
    }).plugins;
    expect(refused).toEqual({
      id: '',
      file: 'evil.rbplugin',
      verified: false,
      name: 'evil.rbplugin',
      version: '',
      description: '',
      declared: { name: 'SEO', version: '9.9.9', claimsIdOf: 'SEO' },
      source: 'project',
      signature: { state: 'signed' },
      state: 'error',
      commands: 0,
    });
    if (refused === undefined) throw new Error('row');
    expect(switchable(refused)).toBe(false);
  });

  it('switches only verified plugins that load or are switched off', () => {
    const [seo, , snippets, broken] = parseInstalled(SAMPLE_INSTALLED).plugins;
    if (seo === undefined || snippets === undefined || broken === undefined) throw new Error('sample');
    expect(switchable(seo)).toBe(true);
    expect(switchable(snippets)).toBe(true);
    expect(switchable(broken)).toBe(false);
    expect(switchable({ ...seo, state: 'blocked' })).toBe(false);
    expect(switchable({ ...seo, state: 'error' })).toBe(false);
  });

  it('drops what it does not understand instead of rendering it', () => {
    const list = parseInstalled({
      loading: true,
      folders: { bundled: 7, elsewhere: '/x' },
      plugins: [
        null,
        'seo',
        { id: '' },
        { id: 'a', verified: true, source: 'cloud', state: 'active' },
        { id: 'b', verified: true, source: 'user', state: 'flying' },
        { id: 'c', verified: true, source: 'user', state: 'error', signature: { state: 'golden' }, commands: -4, error: { code: 'x' } },
        { id: 'd', source: 'user', state: 'error' },
      ],
    });
    expect(list.loading).toBe(true);
    expect(list.folders).toEqual({});
    expect(list.plugins).toEqual([
      {
        id: 'c',
        file: '',
        verified: true,
        name: 'c',
        version: '',
        description: '',
        source: 'user',
        signature: { state: 'unknown' },
        state: 'error',
        commands: 0,
        error: { code: 'x', message: 'x' },
      },
    ]);
    expect(parseInstalled(undefined).plugins).toEqual([]);
    expect(parseInstalled([1, 2]).plugins).toEqual([]);
  });
});

describe('the helpers of a row', () => {
  it('filters the shortcut editor by the plugin\'s command prefix', () => {
    expect(commandQuery('seo')).toBe('plugin.seo.');
    // A command id carries the plugin id with `_` for `-` (manifest `command_id`).
    expect(commandQuery('color-picker')).toBe('plugin.color_picker.');
  });

  it('names the next start only when the list and the load disagree', () => {
    const [seo, , snippets, broken] = parseInstalled(SAMPLE_INSTALLED).plugins;
    if (seo === undefined || snippets === undefined || broken === undefined) throw new Error('sample');
    expect(restartKey(seo, [])).toBeNull();
    expect(restartKey(seo, ['seo'])).toBe('settings.plugins.restart_disable');
    expect(restartKey(snippets, ['snippets'])).toBeNull();
    expect(restartKey(snippets, [])).toBe('settings.plugins.restart_enable');
    expect(restartKey(broken, ['broken', 'seo', ''])).toBeNull();
  });
});
