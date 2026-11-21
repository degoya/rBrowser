// The page copy of the element path in src/collect.js (a classic script `evaluate` runs, built by
// no bundler) held to the shared cases (tests/fixtures/element-path/cases.json, RB-094-10), as
// src/path.rs is: the paths of the rendered images must be the ones the Rust side builds from the
// document. collect.js reads the light DOM only (querySelectorAll, outerHTML), so the shadow cases
// are not its.
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { caseTarget, elementPathCases } from '../../../web-common/src/element-path-cases';

function pathOfCollect(): (element: Element) => string {
  const source = readFileSync(join(dirname(fileURLToPath(import.meta.url)), '../../src/collect.js'), 'utf8');
  const block = /\/\/ element-path: begin\n([\s\S]*?)\n\s*\/\/ element-path: end/.exec(source)?.[1];
  if (!block) throw new Error('src/collect.js has no element-path block');
  return new Function(`${block}\nreturn pathOf;`)() as (element: Element) => string;
}

describe('the element path of src/collect.js', () => {
  const pathOf = pathOfCollect();
  const cases = elementPathCases().filter((entry) => !entry.shadow);

  it.each(cases.map((entry) => [entry.name, entry] as const))('%s', (_name, entry) => {
    expect(pathOf(caseTarget(entry))).toBe(entry.path);
  });
});
