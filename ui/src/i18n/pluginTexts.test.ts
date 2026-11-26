import { afterEach, describe, expect, it } from 'vitest';
import { createShellI18n } from './index';
import { pluginTextsOf, takePluginTexts } from './pluginTexts';

describe('the plugin texts the core sends (RB-080-82)', () => {
  afterEach(() => {
    // A fresh catalogue for the next test.
    createShellI18n();
  });

  it('takes the texts beside the command list and the settings schema', () => {
    expect(
      pluginTextsOf({
        type: 'commands',
        leader: 'Ctrl+Shift+Space',
        commands: [],
        texts: { 'plugin.overlays.commands.content_chaos': 'Toggle content chaos' },
      }),
    ).toEqual({ 'plugin.overlays.commands.content_chaos': 'Toggle content chaos' });
    expect(
      pluginTextsOf({
        type: 'settings',
        values: [],
        schema: { version: 1, texts: { 'plugin.seo.settings.lang.label': 'Language' } },
      }),
    ).toEqual({ 'plugin.seo.settings.lang.label': 'Language' });
    expect(pluginTextsOf({ type: 'ack' })).toBeNull();
  });

  it('takes only plugin keys with text values: a core key cannot be overwritten', () => {
    expect(
      pluginTextsOf({
        type: 'commands',
        leader: 'Ctrl+Shift+Space',
        commands: [],
        texts: { 'commands.sync.toggle': 'Hijacked', 'plugin.x.commands.a': 'A', 'plugin.x.commands.b': 3 } as never,
      }),
    ).toEqual({ 'plugin.x.commands.a': 'A' });
  });

  it('puts them in the catalogue of the window, so t() finds them', () => {
    const i18n = createShellI18n();
    const t = i18n.global.t as (key: string) => string;
    // A plugin key is not in the core's catalogue: alone it shows itself.
    const title = 'plugin.image-overlay.commands.add_design';
    expect(t(title)).toBe(title);
    takePluginTexts({
      type: 'commands',
      leader: 'Ctrl+Shift+Space',
      commands: [],
      texts: {
        'plugin.image-overlay.commands.add_design': 'Image Overlay: Add Design…',
        'commands.sync.toggle': 'Hijacked',
      },
    });
    expect(t(title)).toBe('Image Overlay: Add Design…');
    expect(t('commands.sync.toggle')).toBe('Switch sync on or off');
  });
});

describe('the plugin texts never reach a prototype (security review c20)', () => {
  it('skips a key that walks into constructor, prototype or __proto__', () => {
    const i18n = createShellI18n();
    takePluginTexts({
      type: 'settings',
      values: [],
      schema: {
        version: 1,
        texts: {
          'plugin.demo.settings.constructor.prototype.polluted': 'yes',
          'plugin.demo.__proto__.polluted': 'yes',
          'plugin.demo.settings.prototype.polluted': 'yes',
          'plugin.demo.settings.depth.label': 'Depth',
        },
      },
    });
    expect((Object.prototype as Record<string, unknown>).polluted).toBeUndefined();
    expect(({} as Record<string, unknown>).polluted).toBeUndefined();
    const t = i18n.global.t as (key: string) => string;
    const label = 'plugin.demo.settings.depth.label';
    expect(t(label)).toBe('Depth');
    expect(
      pluginTextsOf({
        type: 'commands',
        leader: 'Ctrl+Shift+Space',
        commands: [],
        texts: { 'plugin.demo.constructor.prototype.x': 'no', 'plugin.demo.commands.a': 'A' },
      }),
    ).toEqual({ 'plugin.demo.commands.a': 'A' });
  });
});
