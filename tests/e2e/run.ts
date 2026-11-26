// The end-to-end runner (RB-099-02), started by scripts/e2e-windows.sh with the Node of .nvmrc
// (it runs TypeScript as it is):
//
//   node tests/e2e/run.ts --list [--only K4,K10] [--native] [--allow-missing firefox]
//   node tests/e2e/run.ts --app <staged app folder> [--native] [--only ...] [--report PATH]
//                         [--work DIR] [--keep] [--allow-missing firefox,webkit]
//
// It serves the fixtures, copies the projects into the work folder, starts the staged app on
// one project after the other (the portable mode with the seeded settings, lib/app.ts), runs
// each case and writes the report (report.json, report.md, logs/<id>.log beside it).
// Exit: 0 every criterion passed (or was allowed to skip), 1 one failed, 2 the run could not
// start (arguments, the app folder, the fixture server's ports).

import { appendFileSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';

import { installOf, prepareData, rbrowserProcesses, windowsPath } from './lib/app.ts';
import { parseArgs } from './lib/args.ts';
import type { Options } from './lib/args.ts';
import { failed } from './lib/case.ts';
import type { Case } from './lib/case.ts';
import { workspaceVersion } from './lib/checks.ts';
import type { Context } from './lib/context.ts';
import { PROJECT_SOURCE, grouped, planText, select } from './lib/plan.ts';
import { stageProject } from './lib/projects.ts';
import { CASES } from './lib/registry.ts';
import { buildReport, exitCode, redact, renderMarkdown } from './lib/report.ts';
import type { Entry } from './lib/report.ts';
import { startFixtureServer } from './lib/server.ts';
import type { FixtureServer } from './lib/server.ts';
import { openSession } from './lib/session.ts';

const ROOT = resolve(import.meta.dirname, '../..');
/** 18766 the fixtures' own port (project-perf, the e2e project), 18765 the older projects'. */
const PORTS = [18766, 18765];

function fixtureServer(work: string): Promise<FixtureServer> {
  const site = join(ROOT, 'tests/fixtures/site');
  return startFixtureServer({
    root: site,
    ports: PORTS,
    mounts: [
      { prefix: '/analyzer/', dir: join(ROOT, 'tests/fixtures/analyzer') },
      { prefix: '/reload/', dir: join(work, 'e2e-project', 'www') },
      { prefix: '/hreflang-broken/', dir: join(site, 'seo/hreflang-broken'), headersFile: 'headers.json' },
      { prefix: '/hreflang-ok/', dir: join(site, 'seo/hreflang-ok'), headersFile: 'headers.json' },
    ],
    rewriteOrigin: 'https://fixture.test',
  });
}

async function runCase(entry: Case, ctx: Omit<Context, 'log'>, logs: string): Promise<Entry> {
  const file = join(logs, `${entry.id}.log`);
  writeFileSync(file, '');
  const log = (line: string) => appendFileSync(file, `${redact(line)}\n`);
  process.stdout.write(`==> ${entry.id} ${entry.title}\n`);
  const started = Date.now();
  let outcome;
  try {
    outcome = await entry.run({ ...ctx, log });
  } catch (error) {
    const message = redact(error instanceof Error ? error.message : String(error));
    log(`error: ${message}`);
    outcome = failed(message);
  }
  const durationMs = Date.now() - started;
  process.stdout.write(`    ${outcome.status}${outcome.error === undefined ? '' : `: ${outcome.error}`}\n`);
  return {
    id: entry.id,
    title: entry.title,
    threshold: entry.threshold,
    durationMs,
    log: relative(dirname(logs), file),
    ...outcome,
  };
}

async function run(options: Options): Promise<number> {
  const groups = grouped(select(CASES, options.only));
  if (options.app === null) {
    process.stderr.write('run.ts: --app names the staged app folder (scripts/e2e-windows.sh stages it)\n');
    return 2;
  }
  const install = installOf(resolve(options.app), options.mode);
  prepareData(install);
  const reportFile = resolve(ROOT, options.report);
  const logs = join(dirname(reportFile), 'logs');
  const screenshots = join(dirname(reportFile), 'screenshots');
  const work = resolve(ROOT, options.work);
  for (const dir of [logs, work]) rmSync(dir, { recursive: true, force: true });
  for (const dir of [logs, screenshots, work]) mkdirSync(dir, { recursive: true });
  const projects = new Map(
    groups
      .filter((group) => group.project !== 'none')
      .map((group) => [group.project, stageProject(ROOT, work, PROJECT_SOURCE[group.project])]),
  );
  // The hot reload mount serves the copy; without the e2e project it stays empty.
  if (!projects.has('e2e')) mkdirSync(join(work, 'e2e-project', 'www'), { recursive: true });
  const server = await fixtureServer(work);
  const started = new Date();
  const entries: Entry[] = [];
  try {
    for (const [index, group] of groups.entries()) {
      const base = {
        mode: options.mode,
        root: ROOT,
        work,
        screenshots,
        server,
        install,
        session: null,
        processesBefore: 0,
        allowMissing: options.allowMissing,
      };
      const project = projects.get(group.project);
      if (project === undefined) {
        for (const entry of group.cases) entries.push(await runCase(entry, base, logs));
        continue;
      }
      const processesBefore = rbrowserProcesses();
      const startLog = join(logs, `start-${group.project}.log`);
      let session;
      try {
        session = await openSession(install, project, (line) => appendFileSync(startLog, `${redact(line)}\n`));
      } catch (error) {
        const message = `the app did not start on ${PROJECT_SOURCE[group.project]}: ${error instanceof Error ? error.message : String(error)}`;
        for (const entry of group.cases) {
          entries.push({ id: entry.id, title: entry.title, threshold: entry.threshold, durationMs: 0, log: relative(dirname(logs), startLog), ...failed(message) });
        }
        continue;
      }
      try {
        for (const entry of group.cases) entries.push(await runCase(entry, { ...base, session, processesBefore }, logs));
      } finally {
        session.close(options.keep && index === groups.length - 1);
      }
    }
  } finally {
    await server.close();
  }
  const report = buildReport(entries, {
    mode: options.mode,
    app: windowsPath(install.dir, options.mode),
    version: workspaceVersion(readFileSync(join(ROOT, 'Cargo.toml'), 'utf8')),
    started,
    durationMs: Date.now() - started.getTime(),
    only: options.only,
  });
  writeFileSync(reportFile, `${JSON.stringify(report, null, 2)}\n`);
  writeFileSync(reportFile.replace(/\.json$/, '') + '.md', renderMarkdown(report));
  const { passed, failed: failedCount, skipped } = report.summary;
  process.stdout.write(`==> ${passed} passed, ${failedCount} failed, ${skipped} skipped; report ${relative(ROOT, reportFile)}\n`);
  return exitCode(report);
}

async function main(argv: readonly string[]): Promise<number> {
  let options: Options;
  try {
    options = parseArgs(argv, CASES.map((entry) => entry.id));
  } catch (error) {
    process.stderr.write(`run.ts: ${error instanceof Error ? error.message : String(error)}\n`);
    return 2;
  }
  if (options.list) {
    process.stdout.write(planText(grouped(select(CASES, options.only)), options));
    return 0;
  }
  try {
    return await run(options);
  } catch (error) {
    process.stderr.write(`run.ts: ${redact(error instanceof Error ? error.message : String(error))}\n`);
    return 2;
  }
}

process.exitCode = await main(process.argv.slice(2));
