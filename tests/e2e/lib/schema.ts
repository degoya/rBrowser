// K5's second half: the config `rbrowser init` wrote holds to schema/config.v1.json (JSON Schema
// 2020-12, written by schemars). Loaded on demand, so `--list` needs no installed packages.

import { readFileSync } from 'node:fs';
import { join } from 'node:path';

/** The schema errors of `config` (`<path> <message>`), none when it is valid. */
export async function configErrors(root: string, config: unknown): Promise<string[]> {
  const { Ajv2020 } = await import('ajv/dist/2020.js');
  // schemars writes formats such as uint32 that Ajv does not know: types and ranges still hold.
  const ajv = new Ajv2020({ strict: false, allErrors: true, validateFormats: false });
  const schema = JSON.parse(readFileSync(join(root, 'schema/config.v1.json'), 'utf8')) as object;
  const validate = ajv.compile(schema);
  if (validate(config)) return [];
  return (validate.errors ?? []).map((error) => `${error.instancePath || '/'} ${error.message ?? 'invalid'}`);
}
