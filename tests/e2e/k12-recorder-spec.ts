// K12 (RB-050-02): a `.spec.ts` exported by the recorder passes `npx playwright test` against the
// fixture site in every breakpoint project. tests/e2e/recorder/run.sh does exactly that with the
// export of a prepared recording (its generated/ files are held to the plugin's output by the
// plugin's own tests); this case runs it and reads Playwright's summary.

import { spawn } from 'node:child_process';
import { join } from 'node:path';

import { failed, measure, outcome } from './lib/case.ts';
import type { Case } from './lib/case.ts';

export const k12: Case = {
  id: 'K12',
  title: 'Recorder export passes Playwright',
  threshold: 'npx playwright test green in every breakpoint project',
  project: 'none',
  needs: [],
  jobs: ['RB-050-02'],
  async run(ctx) {
    let output = '';
    const code = await new Promise<number | null>((resolve, reject) => {
      const child = spawn('bash', [join(ctx.root, 'tests/e2e/recorder/run.sh')], {
        cwd: ctx.root,
        stdio: ['ignore', 'pipe', 'pipe'],
        windowsHide: true,
      });
      const keep = (chunk: Buffer) => {
        const text = chunk.toString('utf8');
        output += text;
        ctx.log(text.trimEnd());
      };
      child.stdout.on('data', keep);
      child.stderr.on('data', keep);
      child.on('error', reject);
      child.on('close', resolve);
    });
    const passed = Number(/(\d+) passed/.exec(output)?.[1] ?? 0);
    const failedTests = Number(/(\d+) failed/.exec(output)?.[1] ?? 0);
    if (passed === 0 && code !== 0) return failed(`tests/e2e/recorder/run.sh ended with ${String(code)} before a test ran`);
    return outcome([
      measure('run.sh exit', code, '0', code === 0),
      measure('tests passed', passed, '> 0', passed > 0),
      measure('tests failed', failedTests, '= 0', failedTests === 0),
    ]);
  },
};
