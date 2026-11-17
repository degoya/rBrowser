/// <reference path="../panel/highlight.d.ts" />
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { beforeAll, describe, expect, it } from 'vitest';
import { tokenizeJson } from '../../../ui/src/settings/jsonTokens';
import { runClassicScripts } from './harness';

// Review of RB-080-65: the settings window's JSON view (ui/src/settings/jsonTokens.ts,
// RB-080-42) and the panels' highlighter colour JSON alike - the same text the same kinds.
// Punctuation and space are uncoloured in the panels and dimmed in the shell; they are left out.

const PLUGINS = join(dirname(fileURLToPath(import.meta.url)), '..', '..');

const CORPUS = [
  '{\n  "ui": {\n    "theme": "dark",\n    "syntaxHighlight": true\n  },\n  "n": -1.5e3,\n  "none": null\n}\n',
  '[1, 2.25, -0, 1E+2, "a\\"b", "\\\\", false, {"k": ["v", {"deep": null}]}]',
  '{"html": "</script><img src=x onerror=alert(1)>", "empty": "", "u": "\\u00e9"}',
  '{"key" :  "spaced", "x":{}}',
  '"just a string"',
  '{"open": "never closed',
];

/** The panel kinds of the shell's. */
const SHELL_KIND: Record<string, string> = { key: 'key', string: 'string', number: 'number', boolean: 'literal', null: 'literal' };

beforeAll(() => {
  runClassicScripts(['highlight-tokens.js', 'highlight.js'].map((file) => readFileSync(join(PLUGINS, 'web-common', 'panel', file), 'utf8')));
});

describe('JSON in the shell and in the panels', () => {
  it.each(CORPUS)('colours %j alike', (text) => {
    const shell = tokenizeJson(text)
      .filter((token) => token.kind in SHELL_KIND)
      .map((token) => `${SHELL_KIND[token.kind]}:${token.text}`);
    const panels = CodeTokens.tokenize(text, 'json')
      .filter((token) => token.kind !== 'plain' && token.kind !== 'punct')
      .map((token) => `${token.kind}:${token.text}`);
    expect(panels).toEqual(shell);
  });
});
