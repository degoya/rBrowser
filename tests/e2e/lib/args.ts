// The runner's command line (tests/e2e/run.ts), as scripts/e2e-windows.sh hands it over.

export type Mode = 'wsl' | 'native';
export type Engine = 'firefox' | 'webkit';

export interface Options {
  /** The criteria asked for (`--only K4,K10`), upper case in the order given; null is all. */
  only: string[] | null;
  /** Print the plan and start nothing. */
  list: boolean;
  /** `wsl`: the app on Windows, this runner in WSL; `native`: both on Windows (Git Bash). */
  mode: Mode;
  /** Leave the last app running and the work folder in place. */
  keep: boolean;
  /** The engines whose absence skips their criterion instead of failing it. */
  allowMissing: Set<Engine>;
  /** The staged app folder (rbrowser.exe, rbrowser.com, the `portable` marker). */
  app: string | null;
  report: string;
  work: string;
}

const ENGINES: readonly Engine[] = ['firefox', 'webkit'];

export function parseArgs(argv: readonly string[], known: readonly string[]): Options {
  const options: Options = {
    only: null,
    list: false,
    mode: 'wsl',
    keep: false,
    allowMissing: new Set(),
    app: null,
    report: 'artifacts/e2e/report.json',
    work: 'artifacts/e2e/work',
  };
  const args = argv.flatMap((arg) => (arg.startsWith('--') && arg.includes('=') ? arg.split(/=(.*)/s, 2) : [arg]));
  for (let i = 0; i < args.length; i += 1) {
    const name = args[i] ?? '';
    const value = (): string => {
      const next = args[i + 1];
      if (next === undefined || next.startsWith('--')) throw new Error(`${name} needs a value`);
      i += 1;
      return next;
    };
    switch (name) {
      case '--only':
        options.only = only(value(), known);
        break;
      case '--list':
        options.list = true;
        break;
      case '--native':
        options.mode = 'native';
        break;
      case '--keep':
        options.keep = true;
        break;
      case '--allow-missing':
        for (const engine of value().split(',')) {
          const found = ENGINES.find((candidate) => candidate === engine.trim().toLowerCase());
          if (found === undefined) throw new Error('--allow-missing takes firefox or webkit');
          options.allowMissing.add(found);
        }
        break;
      case '--app':
        options.app = value();
        break;
      case '--report':
        options.report = value();
        break;
      case '--work':
        options.work = value();
        break;
      default:
        throw new Error(`unknown argument ${name}`);
    }
  }
  return options;
}

function only(list: string, known: readonly string[]): string[] {
  const ids = list
    .split(',')
    .map((id) => id.trim().toUpperCase())
    .filter((id) => id.length > 0);
  for (const id of ids) {
    if (!known.includes(id)) throw new Error(`unknown criterion ${id} (known: ${known.join(', ')})`);
  }
  return ids;
}
