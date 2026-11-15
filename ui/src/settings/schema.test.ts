import { describe, expect, it } from 'vitest';
import type { JsonValue } from '../ipc/protocol';
import { layoutOf, parseSchema, type SettingsSchema } from './schema';
import { realDefs } from './test/fakeCore';

function schemaOf(defs = realDefs()): SettingsSchema {
  const categories = [...new Set(defs.map((def) => def.category))].map((id) => ({
    id,
    sections: [...new Set(defs.filter((def) => def.category === id).map((def) => def.section))],
  }));
  return { version: 1, categories, defs, project: null };
}

describe('the settings layout', () => {
  it('places every key of the schemas exactly once in its own category', () => {
    const defs = realDefs();
    const rendered = layoutOf(schemaOf(defs)).flatMap((category) =>
      category.sections
        .filter((section) => !section.mirror)
        .flatMap((section) => section.defs.map((def) => `${category.id}:${def.key}`)),
    );
    expect(rendered).toHaveLength(defs.length);
    expect(new Set(rendered).size).toBe(defs.length);
    for (const def of defs) {
      expect(rendered).toContain(`${def.category}:${def.key}`);
    }
  });

  it('orders the keys of a section by x-order and repeats plugin keys under Plugins', () => {
    const defs = realDefs();
    const sync = layoutOf(schemaOf(defs)).find((category) => category.id === 'sync');
    const orders = sync?.sections[0]?.defs.map((def) => def.order) ?? [];
    expect(orders).toEqual([...orders].sort((a, b) => a - b));
    const plugin = { ...defs[0]!, key: 'shot.quality', category: 'capture', section: 'screenshots', plugin: 'shot' };
    const withPlugin = schemaOf([...defs, plugin]);
    withPlugin.categories.push({ id: 'plugins', sections: [] });
    const layout = layoutOf(withPlugin);
    const page = layout.find((c) => c.id === 'plugins')?.sections.find((s) => s.id === 'plugin:shot');
    expect(page?.mirror).toBe(true);
    expect(page?.defs.map((def) => def.key)).toEqual(['shot.quality']);
  });

  it('refuses a payload of another shape', () => {
    expect(() => parseSchema(undefined)).toThrow();
    expect(() => parseSchema({ version: 2 })).toThrow();
    expect(() => parseSchema({ version: 1, categories: [], defs: [{ key: 1 }] } as JsonValue)).toThrow();
    const parsed = parseSchema({
      version: 1,
      categories: [{ id: 'sync', sections: ['sync'] }],
      defs: [],
      project: { root: 'C:\\site', hasLocal: true },
    });
    expect(parsed.project).toEqual({ root: 'C:\\site', hasLocal: true });
  });
});
