// The entry of dist/runner.js. The component (src/script.rs) sends one script per distinct
// served source to a view's isolated world: this bundle (html-validate included) and
// `rbHtmlValidate.start(options)`, all inside one function, so nothing lands in the world's
// globals and nothing reaches the page's main world. The answer is the runner's shape
// (src/shape.ts) or an error with its reason.

import { type BuiltinOptions, validateBuiltin } from './builtin';
import { type FetchLike, type NuOptions, validateNu } from './nu';
import type { Answer, Failed } from './shape';

/** What the component asks for: the engine and its options. */
export type StartOptions =
  | ({ engine: 'builtin' } & BuiltinOptions)
  | ({ engine: 'nu' } & NuOptions);

function reason(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** Validates the source with the engine the options name. */
export async function start(options: StartOptions, fetchImpl?: FetchLike): Promise<Answer | Failed> {
  const engine = options.engine;
  try {
    if (engine === 'builtin') return await validateBuiltin(options);
    if (engine === 'nu') return await validateNu(options, fetchImpl);
    return { engine: 'builtin', error: 'unknown engine' };
  } catch (error) {
    return { engine: engine === 'nu' ? 'nu' : 'builtin', error: reason(error).slice(0, 300) };
  }
}
