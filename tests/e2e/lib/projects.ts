// The projects of the run are copies in the work folder: a case writes into its project (K6
// touches its files), the checkout stays as it is, and each run starts untrusted and clean.

import { cpSync, rmSync } from 'node:fs';
import { basename, join } from 'node:path';

/** Copies `<root>/<source>` to `<work>/<its name>`, replacing an earlier copy; answers the copy. */
export function stageProject(root: string, work: string, source: string): string {
  const copy = join(work, basename(source));
  rmSync(copy, { recursive: true, force: true });
  cpSync(join(root, source), copy, { recursive: true });
  return copy;
}
