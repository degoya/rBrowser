// K8 (RB-060-09): the broken hreflang fixture yields exactly the expected findings, the correct
// one yields no error. The fixtures (tests/fixtures/site/seo/hreflang-*) name the origin
// https://fixture.test; the fixture server serves each set at /hreflang-<set>/ with its
// headers.json and its robots.txt as the origin's, the fixture origin rewritten to its own. The
// expected findings are the plugin's own snapshot (plugins/seo/tests/snapshots/hreflang-*.txt):
// severity, rule, message and page of each finding of the MCP tool `seo_check`.

import { readFileSync } from 'node:fs';
import { join } from 'node:path';

import { measure, outcome } from './lib/case.ts';
import type { Case } from './lib/case.ts';
import { diffKeys, findingKey, parseSnapshot } from './lib/checks.ts';
import type { Finding } from './lib/checks.ts';
import { needSession, site } from './lib/context.ts';
import type { Context } from './lib/context.ts';

async function findings(ctx: Context, set: string): Promise<Finding[]> {
  const session = needSession(ctx);
  ctx.server.alias('/robots.txt', join(ctx.root, `tests/fixtures/site/seo/${set}/robots.txt`));
  try {
    const url = site(ctx, `/${set}/en/`);
    await session.navigateAll(url);
    const answer = await session.mcp.value<{ findings?: Finding[]; truncated?: boolean }>(
      'seo_check',
      { url, scope: 'hreflang' },
      180_000,
    );
    if (answer.truncated === true) ctx.log(`${set}: the answer was cut; the report file holds all`);
    return answer.findings ?? [];
  } finally {
    ctx.server.alias('/robots.txt', null);
  }
}

export const k08: Case = {
  id: 'K8',
  title: 'hreflang findings',
  threshold: 'broken: exactly the snapshot findings; ok: 0 errors',
  project: 'e2e',
  needs: [],
  jobs: ['RB-060-09'],
  async run(ctx) {
    const expected = parseSnapshot(
      readFileSync(join(ctx.root, 'plugins/seo/tests/snapshots/hreflang-broken.txt'), 'utf8'),
    );
    const broken = (await findings(ctx, 'hreflang-broken')).map((finding) => findingKey(finding, ctx.server.origin));
    const { missing, unexpected } = diffKeys(expected, broken);
    for (const key of missing) ctx.log(`missing: ${key}`);
    for (const key of unexpected) ctx.log(`unexpected: ${key}`);
    const ok = await findings(ctx, 'hreflang-ok');
    const errors = ok.filter((finding) => finding.severity === 'error');
    for (const finding of errors) ctx.log(`hreflang-ok error: ${findingKey(finding, ctx.server.origin)}`);
    return outcome([
      measure('broken: missing findings', missing.length, '= 0', missing.length === 0),
      measure('broken: unexpected findings', unexpected.length, '= 0', unexpected.length === 0),
      measure('broken: findings', broken.length, `= ${expected.length}`, broken.length === expected.length),
      measure('ok: errors', errors.length, '= 0', errors.length === 0),
    ]);
  },
};
