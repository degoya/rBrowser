// Loads a plugin's classic scripts and pages into the test's window, as the panel frame, a
// view's isolated world and `evaluate` would. Vitest runs in the plugin's `web/` folder, so the
// plugin's files are its siblings (`../panel`, `../src`, `../page`, `../tests`).
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

/** The plugin's folder: the parent of `web/`, where Vitest runs. */
function pluginDir(): string {
  return join(process.cwd(), '..');
}

/** A file of the plugin, as text (`panel/index.html`, `src/ops.js`, `tests/fixtures/...`). */
export function pluginFile(path: string): string {
  return readFileSync(join(pluginDir(), path), 'utf8');
}

/** Writes a file of the plugin (the committed fixtures, on request). */
export function writePluginFile(path: string, text: string): void {
  writeFileSync(join(pluginDir(), path), text);
}

/** A file of the plugin's `panel/` folder, as text. */
export function panelFile(name: string): string {
  return pluginFile(join('panel', name));
}

/** A page of the repository's fixture site (`tests/fixtures/site/<path>`). */
export function fixturePage(path: string): string {
  return readFileSync(join(pluginDir(), '..', '..', 'tests', 'fixtures', 'site', path), 'utf8');
}

/** Runs a classic script of the plugin in the global scope, as a `<script>` element would. */
export function runScript(path: string): void {
  // An indirect eval runs the script as the page's own <script> would: in the global scope.
  (0, eval)(pluginFile(path));
}

/** A script of one function, run as the component runs it: `(<function>)(<options>)`. */
// eslint-disable-next-line @typescript-eslint/no-explicit-any -- what the page script answers is the test's to check
export function collect(path: string, options: unknown): any {
  return (0, eval)(`(${pluginFile(path).trim()})(${JSON.stringify(options)})`);
}

/** The `<body>` of `html`, without its `<script>` elements. */
export function bodyOf(html: string): string {
  const body = /<body>([\s\S]*)<\/body>/.exec(html)?.[1] ?? '';
  return body.replace(/<script[\s\S]*?<\/script>/g, '');
}

/** Puts a page's `<style>` and `<body>` into the test's document. */
export function loadPage(html: string): void {
  const style = /<style>([\s\S]*?)<\/style>/.exec(html)?.[1] ?? '';
  document.head.innerHTML = `<style>${style}</style>`;
  document.body.innerHTML = bodyOf(html);
}

/** The panel page's body, then its classic scripts in order (`panel/model.js`, ...). */
export function loadPanel(scripts: string[], page = 'panel/index.html'): void {
  document.body.innerHTML = bodyOf(pluginFile(page));
  for (const script of scripts) runScript(script);
}

/** The model script, freshly run; `model` answers the global it defined. */
export function loadModel<T>(model: () => T, script = 'panel/model.js'): T {
  runScript(script);
  return model();
}
