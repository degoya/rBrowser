// The entry "Add" starts in a list or a map of the settings (RB-090-61): one its schema accepts,
// so the core's check never refuses it before the person has typed anything (the owner's
// `/0/kind: Not one of the allowed values.` on a new device preset). A schema's `x-new` names a
// useful start (a device preset's kind, viewport and pixel ratio; a breakpoint's size); every
// required field it leaves out gets its `default`, the first allowed value of a choice, the
// minimum of a number, an empty text or list, an object's own required fields. An `id` is made
// unique among the entries, since ids are referenced (a breakpoint's by the grid's cells).

import type { JsonValue } from '../ipc/protocol';
import { isObject } from '../../../plugins/web-common/src/is-object';
import { baseSchema, enumOptions, kindOf, propertiesOf } from './kinds';
import { uniqueName } from './listOps';
import type { JsonSchema } from './schema';

/** The id a new entry takes when its template names none. */
const NEW_ID = 'new';
/** How deep required objects are followed; the settings nest three levels at most. */
const MAX_DEPTH = 8;

/** A new entry for a list of `items` that its schema accepts, its `id` unique among `siblings`. */
export function newItem(items: JsonSchema, siblings: readonly JsonValue[] = []): JsonValue {
  const item = fresh(items, 0);
  if (isObject(item) && typeof item.id === 'string') {
    const taken = new Set(
      siblings.flatMap((other) => (isObject(other) && typeof other.id === 'string' ? [other.id] : [])),
    );
    item.id = uniqueName(item.id === '' ? NEW_ID : item.id, taken);
  }
  return item;
}

function clone(value: JsonValue): JsonValue {
  return JSON.parse(JSON.stringify(value)) as JsonValue;
}

function fresh(schema: JsonSchema, depth: number): JsonValue {
  const template = schema['x-new'];
  if (template !== undefined && !isObject(template)) {
    return clone(template);
  }
  if (template === undefined && schema.default !== undefined) {
    return clone(schema.default);
  }
  const choices = enumOptions(schema);
  if (choices !== null && choices[0] !== undefined) {
    return choices[0].value;
  }
  const base = baseSchema(schema);
  switch (kindOf(base)) {
    case 'boolean':
      return false;
    case 'integer':
    case 'number':
      return typeof base.minimum === 'number' ? base.minimum : 0;
    case 'object':
      return depth > MAX_DEPTH ? {} : fields(base, isObject(template) ? template : {}, depth);
    case 'objectMap':
    case 'choiceMap':
      return {};
    case 'multiEnum':
    case 'pickList':
    case 'stringList':
    case 'objectList':
      return [];
    case 'json':
      return jsonStart(base, depth);
    default:
      return '';
  }
}

/** The fields of the first shape of a choice (`oneOf: [{ required: ["path"] }, ...]`, a bookmark). */
function shapeRequired(schema: JsonSchema): JsonValue[] {
  const branches = [schema.oneOf, schema.anyOf].flatMap((list) => (Array.isArray(list) ? list : []));
  const first = branches.find(isObject);
  return isObject(first) && Array.isArray(first.required) ? first.required : [];
}

/** The template's fields, then every required field it leaves out. */
function fields(schema: JsonSchema, template: Record<string, unknown>, depth: number): JsonValue {
  const item = clone(template as JsonValue) as { [key: string]: JsonValue };
  const required = [...(Array.isArray(schema.required) ? schema.required : []), ...shapeRequired(schema)];
  for (const [name, field] of propertiesOf(schema)) {
    if (required.includes(name) && !(name in item)) {
      item[name] = fresh(field, depth + 1);
    }
  }
  return item;
}

/** A value the window edits as JSON: the first branch of a choice of shapes, else by its type. */
function jsonStart(schema: JsonSchema, depth: number): JsonValue {
  for (const combinator of ['anyOf', 'oneOf'] as const) {
    const branches = schema[combinator];
    const first = Array.isArray(branches)
      ? branches.find((branch) => isObject(branch) && branch.type !== 'null')
      : undefined;
    if (isObject(first)) {
      return fresh(first as JsonSchema, depth + 1);
    }
  }
  const type = Array.isArray(schema.type) ? schema.type.find((t) => t !== 'null') : schema.type;
  switch (type) {
    case 'array':
      return [];
    case 'object':
      return {};
    case 'null':
      return null;
    default:
      return '';
  }
}
