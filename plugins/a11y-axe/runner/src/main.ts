// The entry of dist/runner.js. The component (src/script.rs) sends one script per view and run
// to the view's isolated world: axe-core's own file, this bundle, and `rbA11yAxe.start(options)`
// - all inside one function, so nothing but axe's own `window.axe` lands in the world's globals,
// and that is removed again when the run ends. axe never reaches the page's main world.

import { type AxeLike, type RunOptions, type Answer, runAxe } from './run';

/** The answer of a run that failed: axe threw (an invalid selector) or was missing. */
export interface Failed {
  error: string;
}

type Global = { axe?: AxeLike };

/** Runs the axe the script loaded just before, then removes it from the world's globals. */
export async function start(options: RunOptions): Promise<Answer | Failed> {
  const global = globalThis as unknown as Global;
  try {
    const axe = global.axe;
    if (!axe) return { error: 'axe-core did not load' };
    return await runAxe(axe, options, document);
  } catch (error) {
    return { error: error instanceof Error ? error.message : String(error) };
  } finally {
    delete global.axe;
  }
}
