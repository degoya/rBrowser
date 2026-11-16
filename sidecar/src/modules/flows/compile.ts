// A flow file as a function (RB-040-07). Node strips the TypeScript types (positions kept, so
// lines stay those of the file), and the code becomes the body of an async function whose
// parameters are the injected globals: top-level `await` works and no import is needed. The
// script is compiled with the flow's path as its file name, so a stack names the flow's lines.
// `import` statements are refused (a function body has none), and so is `import()`: the script
// has no dynamic-import callback. Strict mode keeps views from sharing accidental globals.

import { stripTypeScriptTypes } from 'node:module';
import { Script } from 'node:vm';

/** The injected globals, in the order of the function's parameters. */
export const GLOBALS = ['page', 'view', 'expect', 'env', 'step'] as const;

/** Everything before the flow's first line, on that line (so line numbers hold). */
export const PREFIX = `(async function (${GLOBALS.join(', ')}) { 'use strict'; `;

export type FlowFunction = (...globals: unknown[]) => Promise<unknown>;

/** A flow that does not parse: the message and, when found, the line. */
export class FlowSyntaxError extends SyntaxError {
  readonly line: number | null;

  constructor(message: string, line: number | null) {
    super(message);
    this.name = 'SyntaxError';
    this.line = line;
  }
}

/** Compiles `source` as the flow `file`; a syntax error is a [`FlowSyntaxError`]. */
export function compileFlow(source: string, file: string, strip: (code: string) => string = stripTypes): FlowFunction {
  let code: string;
  try {
    code = strip(source);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new FlowSyntaxError(message, syntaxLine(source, message, strip));
  }
  try {
    return new Script(`${PREFIX}${code}\n})`, { filename: file }).runInThisContext() as FlowFunction;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    const stack = error instanceof Error ? (error.stack ?? '') : '';
    // V8 starts a syntax error's stack with `<file>:<line>`.
    const match = stack.startsWith(`${file}:`) ? /^(\d+)/.exec(stack.slice(file.length + 1)) : null;
    throw new FlowSyntaxError(message, match?.[1] === undefined ? null : Number(match[1]));
  }
}

/**
 * The line of a syntax error the type stripper names without a position: the first line after
 * which the code up to it fails with the same message (a cut-off block fails with another one).
 */
export function syntaxLine(source: string, message: string, strip: (code: string) => string): number | null {
  const lines = source.split('\n');
  for (let count = 1; count <= lines.length; count += 1) {
    try {
      strip(lines.slice(0, count).join('\n'));
    } catch (error) {
      if (error instanceof Error && error.message === message) {
        return count;
      }
    }
  }
  return null;
}

function stripTypes(code: string): string {
  return stripTypeScriptTypes(code, { mode: 'strip' });
}

/** The column in the file of `column` on `line` of the compiled script. */
export function fileColumn(line: number, column: number): number {
  return line === 1 ? Math.max(1, column - PREFIX.length) : column;
}
