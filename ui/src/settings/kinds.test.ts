import { describe, expect, it } from 'vitest';
import {
  boundsOf,
  enumOptions,
  isNullable,
  itemsOf,
  kindOf,
  mapValuesOf,
  optionsOf,
  pathInputOf,
  propertiesOf,
} from './kinds';
import type { JsonSchema } from './schema';
import { newItem } from './newItem';
import { realDefs } from './test/fakeCore';

const defs = new Map(realDefs().map((def) => [def.key, def]));

function kind(key: string): string {
  const def = defs.get(key);
  if (def === undefined) {
    throw new Error(`no ${key}`);
  }
  return kindOf(def.schema, def.key);
}

describe('the field renderers', () => {
  it('picks a control for every kind the schema has', () => {
    expect(kind('sync.scroll')).toBe('boolean');
    expect(kind('logs.maxFiles')).toBe('integer');
    expect(kind('emulation.dpr')).toBe('number');
    expect(kind('startUrl')).toBe('string');
    expect(kind('engines.nodePath')).toBe('path');
    expect(kind('engines.firefox.path')).toBe('path');
    expect(kind('ui.theme')).toBe('enum');
    expect(kind('logs.level')).toBe('enum');
    expect(kind('emulation.colorScheme')).toBe('enum');
    expect(kind('reload.watch')).toBe('stringList');
    expect(kind('breakpoints')).toBe('objectList');
    expect(kind('environments')).toBe('objectList');
    expect(kind('layout')).toBe('object');
    expect(kind('emulation.headers')).toBe('json');
  });

  it('edits a map of objects (the layout presets) entry by entry, not as JSON (RB-080-13)', () => {
    const layouts = defs.get('layouts')!.schema;
    const presets = propertiesOf(layouts).find(([name]) => name === 'presets')?.[1] ?? {};
    expect(kindOf(presets, 'presets')).toBe('objectMap');
    expect(kindOf(mapValuesOf(presets))).toBe('object');
    expect(newItem(mapValuesOf(presets))).toEqual({});
    // A map of plain values stays JSON.
    expect(kindOf({ type: 'object', additionalProperties: { type: 'string' } })).toBe('json');
  });

  it('every key has a control', () => {
    for (const def of defs.values()) {
      expect(kindOf(def.schema, def.key)).toBeTruthy();
    }
  });

  it('reads choices, unset and bounds', () => {
    const scheme = defs.get('emulation.colorScheme')!.schema;
    expect(isNullable(scheme)).toBe(true);
    expect(enumOptions(scheme)?.map((o) => o.value)).toEqual(['light', 'dark']);
    expect(boundsOf(defs.get('shortcuts.leaderTimeoutMs')!.schema)).toEqual({ min: 0, max: 10000 });
    const environment = itemsOf(defs.get('environments')!.schema);
    const color = propertiesOf(environment).find(([name]) => name === 'color')?.[1] ?? {};
    expect(kindOf(color, 'color')).toBe('color');
    // A breakpoint's footer colour gets the colour field too (RB-080-47).
    const breakpoint = itemsOf(defs.get('breakpoints')!.schema);
    const footerColor = propertiesOf(breakpoint).find(([name]) => name === 'color')?.[1];
    expect(footerColor).toBeDefined();
    expect(kindOf(footerColor ?? {}, 'color')).toBe('color');
    // What "Add" starts with is `newItem.test.ts` (RB-090-61).
  });

  // RB-090-81: an earlier form the schema still accepts (a branch marked `deprecated`) is no
  // choice the window offers.
  it('offers no deprecated branch as a choice', () => {
    const recordHover: JsonSchema = {
      oneOf: [
        { type: 'string', const: 'menu' },
        { type: 'string', const: 'all' },
        { type: 'string', const: 'off' },
        { type: 'boolean', deprecated: true },
      ],
    };
    expect(enumOptions(recordHover)?.map((o) => o.value)).toEqual(['menu', 'all', 'off']);
    expect(kindOf(recordHover, 'automation.recorder.recordHover')).toBe('enum');
  });

  // RB-080-20: the dialog comes from the schema, not from the name.
  it('reads the native dialog of a path from x-input', () => {
    expect(kindOf({ type: ['string', 'null'], 'x-input': 'file' }, 'engines.firefox.path')).toBe('path');
    expect(kindOf({ type: 'string', 'x-input': 'folder' }, 'dir')).toBe('path');
    expect(kindOf({ type: 'string' }, 'engines.nodePath')).toBe('string');
    expect(pathInputOf({ type: ['string', 'null'], 'x-input': 'folder' })).toBe('folder');
    expect(pathInputOf({ type: 'string', 'x-input': 'url' })).toBeNull();
    expect(pathInputOf({ anyOf: [{ type: 'string', 'x-input': 'file' }, { type: 'null' }] })).toBe('file');
  });

  it('makes a list of allowed values a multi-select', () => {
    expect(kindOf({ type: 'array', items: { type: 'string', enum: ['performance', 'seo'] } })).toBe('multiEnum');
    expect(kindOf({ type: 'array', items: { type: 'string', 'x-options': ['wcag22aa'] } })).toBe('stringList');
    const lighthouse = defs.get('checks.lighthouse')!.schema;
    const categories = propertiesOf(lighthouse).find(([name]) => name === 'categories')?.[1] ?? {};
    expect(kindOf(categories, 'categories')).toBe('multiEnum');
  });

  it('reads the suggested values of a free string', () => {
    expect(optionsOf({ type: ['string', 'null'], 'x-options': ['Win32', 'MacIntel'] })).toEqual(['Win32', 'MacIntel']);
    expect(optionsOf({ type: 'string' })).toEqual([]);
    expect(optionsOf(defs.get('emulation.platform')!.schema)).toContain('Win32');
  });
});
