import { readFileSync } from 'node:fs';
import { join } from 'node:path';

import { describe, expect, it } from 'vitest';

import { configErrors } from './schema.ts';

const root = join(import.meta.dirname, '../../..');

describe('configErrors (K5: a schema-valid config)', () => {
  it('accepts the end-to-end project config', async () => {
    const config: unknown = JSON.parse(readFileSync(join(root, 'tests/fixtures/e2e-project/.rbrowser/config.json'), 'utf8'));
    expect(await configErrors(root, config)).toEqual([]);
  });

  it('names what breaks the schema', async () => {
    const errors = await configErrors(root, { version: 1, breakpoints: [{ id: 'x', width: 'wide', height: 1 }] });
    expect(errors.join('\n')).toContain('/breakpoints/0/width');
  });
});
