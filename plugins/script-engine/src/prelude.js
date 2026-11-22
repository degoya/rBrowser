// The prelude of every script run (RB-098-01): `rb`, `console` and the start and the outcome of
// the run. Evaluated before the script in the same fresh realm; nothing survives the run.
//
// The native `__rb_call(tool, json)` answers "1" + the tool's JSON answer or "0" + the refusal's
// JSON; `__rb_args` and `__rb_target` are the run's inputs as JSON text. All three are taken
// here and removed from the global object.
(function () {
  'use strict';
  const global = globalThis;
  const call = global.__rb_call;
  const argsText = global.__rb_args;
  const targetText = global.__rb_target;
  delete global.__rb_call;
  delete global.__rb_args;
  delete global.__rb_target;

  const stringify = JSON.stringify;
  const parse = JSON.parse;
  const define = Object.defineProperty;
  const toText = String;
  const MAX_LOG_LINES = 200;
  const MAX_LOG_BYTES = 65536;
  const logs = [];
  let logBytes = 0;
  let outcome;

  // The UTF-8 bytes of a code point.
  function utf8(codePoint) {
    return codePoint < 0x80 ? 1 : codePoint < 0x800 ? 2 : codePoint < 0x10000 ? 3 : 4;
  }

  function log(...parts) {
    if (logs.length >= MAX_LOG_LINES || logBytes >= MAX_LOG_BYTES) {
      return;
    }
    const line = parts
      .map((part) => {
        if (typeof part === 'string') {
          return part;
        }
        try {
          const json = stringify(part);
          return json === undefined ? toText(part) : json;
        } catch {
          return toText(part);
        }
      })
      .join(' ');
    // Kept up to MAX_LOG_BYTES of UTF-8 over all lines, cut at a whole character.
    let end = 0;
    for (const character of line) {
      const bytes = utf8(character.codePointAt(0));
      if (logBytes + bytes > MAX_LOG_BYTES) {
        logBytes = MAX_LOG_BYTES;
        break;
      }
      logBytes += bytes;
      end += character.length;
    }
    logs.push(end === line.length ? line : line.slice(0, end));
  }

  // A tool error as the script sees it: an Error named ToolError with the tool and the code.
  function toolError(tool, refusal) {
    let failure = { code: 'tool_failed', message: refusal };
    try {
      failure = parse(refusal);
    } catch {}
    const error = new Error(toText(failure.message ?? failure.error ?? ''));
    error.name = 'ToolError';
    error.code = toText(failure.code ?? 'tool_failed');
    error.tool = tool;
    return error;
  }

  // Every tool by its MCP name: `await rb.navigate({ target: "all", url })`.
  const rb = new Proxy(Object.create(null), {
    get(_, name) {
      if (typeof name !== 'string' || name === 'then') {
        return undefined;
      }
      return async (input) => {
        const answer = call(name, stringify(input === undefined ? {} : input));
        if (answer[0] === '1') {
          return parse(answer.slice(1));
        }
        throw toolError(name, answer.slice(1));
      };
    },
  });

  // The line of the script an error was thrown at; the wrapper adds one line before it.
  function lineOf(stack) {
    if (typeof stack !== 'string') {
      return undefined;
    }
    const match = /\(script:(\d+):\d+\)/.exec(stack);
    if (!match) {
      return undefined;
    }
    const line = Number(match[1]) - 1;
    return line > 0 ? line : undefined;
  }

  function describe(error) {
    const out = { code: 'script.error', name: 'Error', message: '' };
    if (error !== null && (typeof error === 'object' || typeof error === 'function')) {
      try {
        out.name = toText(error.name ?? 'Error');
      } catch {}
      try {
        out.message = toText(error.message ?? '');
      } catch {}
      try {
        if (typeof error.code === 'string') {
          out.code = error.code;
        }
      } catch {}
      try {
        if (typeof error.tool === 'string') {
          out.tool = error.tool;
        }
      } catch {}
      try {
        const line = lineOf(error.stack);
        if (line !== undefined) {
          out.line = line;
        }
      } catch {}
    } else {
      try {
        out.message = toText(error);
      } catch {}
    }
    out.logs = logs;
    return out;
  }

  function finish(value) {
    let json;
    try {
      json = stringify(value);
    } catch (error) {
      const failure = describe(error);
      failure.message = 'the returned value is not JSON: ' + failure.message;
      outcome = '0' + stringify(failure);
      return;
    }
    outcome = '1{"value":' + (json === undefined ? 'null' : json) + ',"logs":' + stringify(logs) + '}';
  }

  const console = { log, info: log, warn: log, error: log, debug: log };
  define(global, 'rb', { value: rb, writable: false, enumerable: false, configurable: false });
  define(global, 'console', { value: console, writable: false, enumerable: false, configurable: false });
  define(global, '__rb_start', {
    value(main) {
      const args = parse(argsText);
      const target = parse(targetText);
      Promise.resolve()
        .then(() => main(args, target))
        .then(finish, (error) => {
          outcome = '0' + stringify(describe(error));
        });
    },
    writable: false,
    enumerable: false,
    configurable: false,
  });
  define(global, '__rb_outcome', {
    value: () => outcome,
    writable: false,
    enumerable: false,
    configurable: false,
  });
})();
