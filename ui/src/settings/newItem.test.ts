import { describe, expect, it } from 'vitest';
import type { JsonValue } from '../ipc/protocol';
import { isObject } from '../../../plugins/web-common/src/is-object';
import { itemsOf, kindOf } from './kinds';
import { newItem } from './newItem';
import type { JsonSchema } from './schema';
import { deepCheck } from './test/deepCheck';
import { realDefs } from './test/fakeCore';

/** A device preset's item schema as the core generates it, before `x-new` (RB-090-61). */
const DEVICE: JsonSchema = {
  type: 'object',
  properties: {
    id: { type: 'string' },
    label: { type: ['string', 'null'] },
    kind: {
      oneOf: [
        { type: 'string', const: 'mobile' },
        { type: 'string', const: 'tablet' },
        { type: 'string', const: 'desktop' },
      ],
    },
    viewport: {
      type: 'object',
      properties: { width: { type: 'integer', minimum: 0 }, height: { type: 'integer', minimum: 0 } },
      required: ['width', 'height'],
    },
    dpr: { type: 'number' },
    touch: { type: 'boolean', default: false },
  },
  required: ['id', 'kind', 'viewport', 'dpr'],
};

describe('a new list entry (RB-090-61)', () => {
  it('gives a choice its first allowed value, never an empty text', () => {
    const item = newItem(DEVICE);
    expect(item).toEqual({ id: 'new', kind: 'mobile', viewport: { width: 0, height: 0 }, dpr: 0 });
    expect(deepCheck(DEVICE, item)).toEqual([]);
    expect(newItem({ type: 'string', enum: ['a', 'b'] })).toBe('a');
  });

  it('starts from the schema x-new template and fills what it leaves out', () => {
    const schema: JsonSchema = {
      ...DEVICE,
      'x-new': { id: 'device', kind: 'tablet', viewport: { width: 390, height: 844 }, dpr: 3 },
    };
    expect(newItem(schema)).toEqual({ id: 'device', kind: 'tablet', viewport: { width: 390, height: 844 }, dpr: 3 });
    const partial: JsonSchema = { ...DEVICE, 'x-new': { id: 'device', dpr: 2 } };
    expect(newItem(partial)).toEqual({ id: 'device', kind: 'mobile', viewport: { width: 0, height: 0 }, dpr: 2 });
  });

  it('takes a default and a minimum for a required number', () => {
    const schema: JsonSchema = {
      type: 'object',
      properties: {
        zoom: { type: 'number', default: 0.5 },
        col: { type: 'integer', minimum: 1 },
        tags: { type: 'array', items: { type: 'string' } },
      },
      required: ['zoom', 'col', 'tags'],
    };
    expect(newItem(schema)).toEqual({ zoom: 0.5, col: 1, tags: [] });
  });

  it('makes an id unique among the entries of the list', () => {
    const schema: JsonSchema = { ...DEVICE, 'x-new': { id: 'device' } };
    const siblings: JsonValue[] = [{ id: 'device' }, { id: 'device-2' }];
    expect(newItem(schema, siblings)).toMatchObject({ id: 'device-3' });
    expect(newItem(DEVICE, [{ id: 'new' }])).toMatchObject({ id: 'new-2' });
  });

  it('every Add of every list and map in the settings yields an entry its schema accepts', () => {
    const lists: [string, JsonSchema][] = [];
    const walk = (path: string, schema: JsonValue): void => {
      if (!isObject(schema)) return;
      const s = schema as JsonSchema;
      const kind = kindOf(s, path);
      if (kind === 'objectList' || kind === 'stringList') lists.push([path, itemsOf(s)]);
      if (kind === 'objectMap' && isObject(s.additionalProperties)) lists.push([path, s.additionalProperties as JsonSchema]);
      for (const nested of ['items', 'additionalProperties'] as const) {
        if (isObject(s[nested])) walk(`${path}[]`, s[nested]);
      }
      for (const branch of [...(Array.isArray(s.anyOf) ? s.anyOf : []), ...(Array.isArray(s.oneOf) ? s.oneOf : [])]) {
        walk(path, branch);
      }
      if (isObject(s.properties)) {
        for (const [name, field] of Object.entries(s.properties)) walk(`${path}.${name}`, field as JsonValue);
      }
    };
    for (const def of realDefs()) walk(def.key, def.schema);
    const keys = lists.map(([path]) => path);
    // The lists of the owner's report and their neighbours are among them.
    expect(keys).toEqual(expect.arrayContaining(['breakpoints', 'devices.items', 'environments', 'layout.cells']));
    for (const [path, schema] of lists) {
      expect([path, deepCheck(schema, newItem(schema))]).toEqual([path, []]);
    }
  });

  it('a new device preset has an allowed kind (the owner report: /0/kind)', () => {
    const def = realDefs().find((candidate) => candidate.key === 'devices.items');
    const items = itemsOf(def?.schema ?? {});
    const item = newItem(items);
    expect(isObject(item) && ['mobile', 'tablet', 'desktop'].includes(String(item.kind))).toBe(true);
    expect(deepCheck(def?.schema ?? {}, [item])).toEqual([]);
  });
});
