import { describe, expect, it } from 'vitest';
import { sampleState } from '../ipc/mockState';
import type { CommandInfo } from '../ipc/protocol';
import type { SettingDef } from '../settings/schema';
import { commandsProvider, settingsProvider } from './providers';
import { createProviderRegistry } from './registry';
import { stateProviders } from './stateProviders';
import { STEP_SET_VIEW, type PaletteProvider, type ProviderContext } from './types';

const t = (key: string, args?: Record<string, string | number>): string =>
  args === undefined ? key : `${key} ${JSON.stringify(args)}`;

function command(id: string, extra: Partial<CommandInfo> = {}): CommandInfo {
  return {
    id,
    titleKey: `commands.${id}`,
    group: 'sync',
    bindings: [],
    defaultBindings: [],
    changed: false,
    mcp: { omitted: 'test' },
    ...extra,
  };
}

function def(key: string): SettingDef {
  return {
    key,
    category: 'sync',
    section: 'scroll',
    order: 1,
    scopeAllowed: ['user'],
    localOnly: false,
    protectedFromMcp: false,
    labelKey: `settings.key.${key}.label`,
    descriptionKey: `settings.key.${key}.description`,
    helpKey: `settings.key.${key}.help`,
    schema: {},
  };
}

function context(extra: Partial<ProviderContext> = {}): ProviderContext {
  return { state: sampleState(), commands: [], settings: null, values: new Map(), t, ...extra };
}

describe('the commands provider', () => {
  it('lists every registry command but the palette with its shortcut and id', () => {
    const entries = commandsProvider.entries(
      context({
        commands: [
          command('sync.toggle', { bindings: ['Ctrl+Shift+Space S S'] }),
          command('app.command_palette'),
          command('plugin.acme.lint', { group: 'plugins' }),
        ],
      }),
    );
    expect(entries.map((e) => e.key)).toEqual(['command:sync.toggle', 'command:plugin.acme.lint']);
    const [sync] = entries;
    expect(sync).toMatchObject({
      scope: 'command',
      title: 'commands.sync.toggle',
      group: 'commands.group.sync',
      shortcut: 'Ctrl+Shift+Space S S',
      commandId: 'sync.toggle',
      keywords: ['sync.toggle'],
      action: { kind: 'send', message: { type: 'runCommand', id: 'sync.toggle' } },
    });
  });
});

describe('the settings provider', () => {
  it('turns every settings key into a deep link', () => {
    const entries = settingsProvider.entries(
      context({
        settings: { version: 1, categories: [], defs: [def('sync.scroll')], project: null },
      }),
    );
    expect(entries).toEqual([
      expect.objectContaining({
        key: 'setting:sync.scroll',
        scope: 'setting',
        title: 'settings.key.sync.scroll.label',
        group: 'settings.category.sync',
        description: 'settings.key.sync.scroll.description',
        keywords: ['sync.scroll'],
        action: { kind: 'send', message: { type: 'openSettings', key: 'sync.scroll' } },
      }),
    ]);
  });

  it('lists nothing without a schema', () => {
    expect(settingsProvider.entries(context())).toEqual([]);
  });
});

describe('the state providers', () => {
  const all = (ctx: ProviderContext) => stateProviders.flatMap((p) => p.entries(ctx));

  it('sets the active view to a breakpoint in the step "Set view to ..."', () => {
    const entries = all(context());
    const tablet = entries.find((e) => e.key === 'set-view:tablet');
    expect(tablet).toMatchObject({
      parent: STEP_SET_VIEW,
      action: { kind: 'send', message: { type: 'setBreakpoint', view: 1, breakpoint: 'tablet' } },
    });
    expect(entries.find((e) => e.key === 'step:set-view')?.action).toEqual({ kind: 'step', step: STEP_SET_VIEW });
  });

  it('focuses a view, switches a layout preset and an environment', () => {
    const entries = all(context());
    expect(entries.find((e) => e.key === 'view:2')).toMatchObject({
      scope: 'view',
      action: { kind: 'send', message: { type: 'setActiveView', view: 2 } },
    });
    expect(entries.find((e) => e.key === 'layout:focus')?.action).toEqual({
      kind: 'send',
      message: { type: 'selectLayoutPreset', id: 'focus' },
    });
    expect(entries.find((e) => e.key === 'environment:staging')?.action).toEqual({
      kind: 'send',
      message: { type: 'selectEnvironment', id: 'staging' },
    });
  });

  it('offers nothing about views when there is no state', () => {
    expect(all(context({ state: null }))).toEqual([]);
  });
});

describe('the provider registry', () => {
  const one: PaletteProvider = {
    id: 'one',
    entries: () => [{ key: 'a', scope: 'other', title: 'A', action: { kind: 'step', step: 's' } }],
  };

  it('collects every provider, the first of a key winning', () => {
    const registry = createProviderRegistry([one]);
    registry.register({ id: 'two', entries: () => [{ ...one.entries(context())[0]!, title: 'B' }] });
    expect(registry.collect(context()).map((e) => e.title)).toEqual(['A']);
  });

  it('removes a provider again and replaces one of the same id', () => {
    const registry = createProviderRegistry();
    const remove = registry.register(one);
    expect(registry.collect(context())).toHaveLength(1);
    remove();
    expect(registry.collect(context())).toHaveLength(0);
    registry.register(one);
    registry.register({ id: 'one', entries: () => [] });
    expect(registry.collect(context())).toHaveLength(0);
  });

  it('skips a provider that throws and asks the query providers per keystroke', () => {
    const registry = createProviderRegistry([
      { id: 'broken', entries: () => { throw new Error('boom'); } },
      {
        id: 'size',
        entries: () => [],
        forQuery: (query) =>
          /^\d+x\d+$/.test(query)
            ? [{ key: `size:${query}`, scope: 'other', title: query, parent: STEP_SET_VIEW, action: { kind: 'step', step: 'x' } }]
            : [],
      },
    ]);
    expect(registry.collect(context())).toEqual([]);
    expect(registry.collectForQuery('500x800', context()).map((e) => e.key)).toEqual(['size:500x800']);
    expect(registry.collectForQuery('tablet', context())).toEqual([]);
  });
});
