// K5 (RB-010-13, RB-030-07): `rbrowser init --yes` proposes exactly the expected widths for the
// Tailwind v3, Tailwind v4 and Bootstrap fixtures and writes a schema-valid config. Each fixture
// is copied into the work folder as a project of its own (its sources beside the stylesheets,
// so the static source reads them - tests/fixtures/analyzer/<fixture>/expected-static.json) and
// served by the fixture server under /analyzer/.

import { readFileSync } from 'node:fs';
import { join } from 'node:path';

import { runCom, windowsPath } from './lib/app.ts';
import { measure, outcome } from './lib/case.ts';
import type { Case } from './lib/case.ts';
import { viewsOf } from './lib/checks.ts';
import { needInstall, site } from './lib/context.ts';
import { stageProject } from './lib/projects.ts';
import { configErrors } from './lib/schema.ts';
import type { Measurement } from './lib/report.ts';

const FIXTURES = ['tailwind3', 'tailwind4', 'bootstrap'];

export const k05: Case = {
  id: 'K5',
  title: 'rbrowser init widths',
  threshold: 'exactly the widths of expected-static.json; schema-valid config',
  project: 'none',
  needs: [],
  jobs: ['RB-010-13', 'RB-030-07'],
  async run(ctx) {
    const install = needInstall(ctx);
    const measurements: Measurement[] = [];
    for (const fixture of FIXTURES) {
      const project = stageProject(ctx.root, join(ctx.work, 'k5'), `tests/fixtures/analyzer/${fixture}`);
      const ran = await runCom(install, [
        'init',
        '--yes',
        '--url',
        site(ctx, `/analyzer/${fixture}/`),
        '--project',
        windowsPath(project, ctx.mode),
      ]);
      ctx.log(`${fixture}: exit ${String(ran.code)}\n${ran.stdout}${ran.stderr}`);
      if (ran.code !== 0) {
        measurements.push(measure(`${fixture} init`, `exit ${String(ran.code)}`, 'exit 0', false));
        continue;
      }
      const config: unknown = JSON.parse(readFileSync(join(project, '.rbrowser/config.json'), 'utf8'));
      const expected = JSON.parse(
        readFileSync(join(ctx.root, `tests/fixtures/analyzer/${fixture}/expected-static.json`), 'utf8'),
      ) as { all: [string, number, number][] };
      const views = viewsOf(config);
      const same = JSON.stringify(views) === JSON.stringify(expected.all);
      measurements.push(
        measure(`${fixture} widths`, views.map(([, width]) => width).join(','), expected.all.map(([, width]) => width).join(','), same),
      );
      if (!same) ctx.log(`${fixture}: got ${JSON.stringify(views)}, expected ${JSON.stringify(expected.all)}`);
      const errors = await configErrors(ctx.root, config);
      measurements.push(measure(`${fixture} schema errors`, errors.length, '= 0', errors.length === 0));
      for (const error of errors) ctx.log(`${fixture}: ${error}`);
    }
    return outcome(measurements);
  },
};
