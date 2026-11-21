// The header's two actions with a result below the tabs (RB-096-11): "Copy as code" and the
// benchmark. Each run is one reactive object the header's controls start and the results area
// shows, so the controls stay fixed above the tabs and the result may be as tall as it is.
import { reactive } from 'vue';
import type { Panel } from '../../../../web-common/panel-ui/src/panel';
import type { Bench } from './checks';
import { type CodeAnswer, type Language, writeClipboard } from './code';
import type { ApiRequest } from './model';

export interface CodeRun {
  language: Language;
  snippet: CodeAnswer | null;
  copied: boolean;
  failure: string | null;
  busy: boolean;
  copy(request: ApiRequest, environment: string | undefined): Promise<void>;
}

/** "Copy as" (RB-096-03): the core writes the code (op `code`), the panel copies and shows it. */
export function createCodeRun(panel: Panel): CodeRun {
  const run: CodeRun = reactive({
    language: 'curl',
    snippet: null,
    copied: false,
    failure: null,
    busy: false,
    async copy(request: ApiRequest, environment: string | undefined) {
      if (run.busy) return;
      run.busy = true;
      run.failure = null;
      try {
        const answer = (await panel.call({ tool: 'panel' }, { op: { op: 'code', request, language: run.language, environment: environment ?? null } })) as CodeAnswer;
        run.snippet = answer;
        run.copied = await writeClipboard(answer.text);
      } catch (error) {
        run.snippet = null;
        run.failure = panel.error(error);
      } finally {
        run.busy = false;
      }
    },
  }) as CodeRun;
  return run;
}

export interface BenchRun {
  compareWith: string | undefined;
  result: Bench | null;
  problem: string | null;
  running: boolean;
  /** Drops the result: another request, another environment. */
  reset(environment: string | undefined): void;
  start(id: string, environment: string | undefined): Promise<void>;
}

/** The benchmark of a saved request (RB-096-04) in the chosen environment and, when one is
 * picked, a second one to compare. */
export function createBenchRun(panel: Panel): BenchRun {
  const run: BenchRun = reactive({
    compareWith: undefined,
    result: null,
    problem: null,
    running: false,
    reset(environment: string | undefined) {
      run.result = null;
      run.problem = null;
      if (run.compareWith === environment) run.compareWith = undefined;
    },
    async start(id: string, environment: string | undefined) {
      if (run.running) return;
      run.running = true;
      run.problem = null;
      const environments = [environment, run.compareWith].filter((name): name is string => typeof name === 'string' && name !== '');
      try {
        run.result = (await panel.call({ tool: 'panel' }, { op: { op: 'bench', id, environments } })) as Bench;
      } catch (error) {
        run.result = null;
        run.problem = panel.error(error);
      } finally {
        run.running = false;
      }
    },
  }) as BenchRun;
  return run;
}
