// What a case gets from the runner (RB-099-02): the mode, the folders, the fixture server, the
// staged app and - for a case with a project - the running session, plus its own log.

import type { AppInstall } from './app.ts';
import type { Engine, Mode } from './args.ts';
import { skipped, failed } from './case.ts';
import type { FixtureServer } from './server.ts';
import type { Session } from './session.ts';
import type { Outcome } from './report.ts';

export interface Context {
  mode: Mode;
  /** The repository root. */
  root: string;
  /** This run's work folder (the projects' copies, files a case writes). */
  work: string;
  /** Where screenshots go (artifacts/e2e/screenshots). */
  screenshots: string;
  server: FixtureServer;
  install: AppInstall | null;
  session: Session | null;
  /** The rbrowser.exe processes that ran before this session's app started. */
  processesBefore: number;
  allowMissing: ReadonlySet<Engine>;
  log(line: string): void;
}

export function needSession(ctx: Context): Session {
  if (ctx.session === null) throw new Error('this case needs a running app');
  return ctx.session;
}

export function needInstall(ctx: Context): AppInstall {
  if (ctx.install === null) throw new Error('this case needs the staged app (scripts/e2e-windows.sh)');
  return ctx.install;
}

/** A missing engine: skipped when `--allow-missing` names it, a failure otherwise. */
export function missingEngine(ctx: Context, engine: Engine, why: string): Outcome {
  return ctx.allowMissing.has(engine)
    ? skipped(`${engine} missing (${why}); allowed by --allow-missing ${engine}`)
    : failed(`${engine} missing: ${why} (--allow-missing ${engine} skips this criterion explicitly)`);
}

/** A URL of the fixture server. */
export function site(ctx: Context, path: string): string {
  return `${ctx.server.origin}${path}`;
}
