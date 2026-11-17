// The core's check of a written value against its schema (`settings_window/deepcheck.rs`), for
// tests: `anyOf`/`oneOf`, `const`, `type`, `enum`, `minimum`/`maximum`, `required`, unknown
// fields where `additionalProperties` is `false`. The paths of the problems it finds, as the core
// reports them (`/0/kind`).

import type { JsonValue } from '../../ipc/protocol';
import type { JsonSchema } from '../schema';
import { isObject } from '../../../../plugins/web-common/src/is-object';

function hasType(value: JsonValue, kind: string): boolean {
  switch (kind) {
    case 'null':
      return value === null;
    case 'boolean':
      return typeof value === 'boolean';
    case 'integer':
      return typeof value === 'number' && Number.isInteger(value);
    case 'number':
      return typeof value === 'number';
    case 'string':
      return typeof value === 'string';
    case 'array':
      return Array.isArray(value);
    case 'object':
      return isObject(value);
    default:
      return true;
  }
}

/** Every problem of `value` against `schema`, as JSON pointers below the value. */
export function deepCheck(schema: JsonValue, value: JsonValue, at = ''): string[] {
  if (!isObject(schema)) return [];
  const s = schema as JsonSchema;
  for (const combinator of ['anyOf', 'oneOf']) {
    const branches = s[combinator];
    if (Array.isArray(branches) && !branches.some((branch) => deepCheck(branch, value, at).length === 0)) {
      return [at];
    }
  }
  if (s.const !== undefined && JSON.stringify(s.const) !== JSON.stringify(value)) return [at];
  const types = typeof s.type === 'string' ? [s.type] : Array.isArray(s.type) ? s.type.map(String) : [];
  if (types.length > 0 && !types.some((kind) => hasType(value, kind))) return [at];
  if (Array.isArray(s.enum) && !s.enum.some((allowed) => JSON.stringify(allowed) === JSON.stringify(value))) {
    return [at];
  }
  const issues: string[] = [];
  if (typeof value === 'number') {
    if (typeof s.minimum === 'number' && value < s.minimum) issues.push(at);
    if (typeof s.maximum === 'number' && value > s.maximum) issues.push(at);
  }
  if (Array.isArray(value)) {
    value.forEach((item, index) => issues.push(...deepCheck(s.items ?? null, item, `${at}/${index}`)));
  } else if (isObject(value)) {
    const properties = isObject(s.properties) ? s.properties : {};
    for (const required of Array.isArray(s.required) ? s.required : []) {
      if (typeof required === 'string' && !(required in value)) issues.push(`${at}/${required}`);
    }
    for (const [name, field] of Object.entries(value)) {
      const own = properties[name] as JsonValue | undefined;
      if (own !== undefined) {
        issues.push(...deepCheck(own, field as JsonValue, `${at}/${name}`));
      } else if (s.additionalProperties === false) {
        issues.push(`${at}/${name}`);
      } else if (isObject(s.additionalProperties)) {
        issues.push(...deepCheck(s.additionalProperties as JsonValue, field as JsonValue, `${at}/${name}`));
      }
    }
  }
  return issues;
}
