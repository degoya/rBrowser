// No plugin panel submits a form (RB-096-13). A panel runs in `<iframe sandbox="allow-scripts">`
// without `allow-forms`, and Chromium refuses a form's submission before it fires `submit`: a
// `<form @submit.prevent>` never runs its handler there ("Blocked form submission to '' because
// the form's frame is sandboxed and the 'allow-forms' permission is not set"), so its OK, Save
// or Add does nothing. The sandbox stays; a panel's form is `PanelForm` (no form element), its
// buttons `type="button"`. This walks every panel's sources - each plugin's Vue components and
// panel scripts, its built-free panel pages, panel-ui and the SDK templates - and names each one
// that renders a form.
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { dirname, join, relative, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const here = dirname(fileURLToPath(import.meta.url));
const plugins = join(here, '..', '..', '..');
const repo = join(plugins, '..');

/** What submits: a form element (`<form>`, Nuxt UI's `UForm`, one made by script) or a submit
 * button, which only a form gives a meaning. `UFormField` is a label and stays. */
const SUBMITS = [/<form[\s>/]/i, /<UForm[\s>/]/, /createElement\(\s*['"]form['"]\s*\)/, /type=["']submit["']/i, /\btype:\s*['"]submit['"]/];

const SKIP = new Set(['node_modules', 'dist', '.nuxt-ui', 'tests', 'target']);

function files(root: string, keep: (path: string) => boolean): string[] {
  let found: string[] = [];
  let names: string[];
  try {
    names = readdirSync(root);
  } catch {
    return found;
  }
  for (const name of names) {
    if (SKIP.has(name)) continue;
    const path = join(root, name);
    if (statSync(path).isDirectory()) found = found.concat(files(path, keep));
    else if (keep(path)) found.push(path);
  }
  return found;
}

const isTest = (path: string): boolean => /\.test\.[jt]s$/.test(path) || path.endsWith('test-setup.ts');
const inPanel = (path: string): boolean => path.split(sep).some((part) => part === 'panel' || part.startsWith('panel.'));

/** Every source a panel is made of: Vue components anywhere in the plugins' web code, scripts and
 * pages of a panel. A page script (forms of the page it reads) is no panel and is left out. */
function panelSources(): string[] {
  const keep = (path: string): boolean =>
    !isTest(path) && (path.endsWith('.vue') || (inPanel(path) && /\.(ts|js|html)$/.test(path)));
  return [...files(plugins, keep), ...files(join(repo, 'sdk', 'templates'), keep)];
}

describe('the plugin panels', () => {
  it('find their sources', () => {
    const sources = panelSources().map((path) => relative(repo, path));
    expect(sources).toContain(join('plugins', 'api-client', 'web', 'src', 'panel', 'App.vue'));
    expect(sources).toContain(join('plugins', 'web-common', 'panel-ui', 'src', 'components', 'PanelForm.vue'));
  });

  it('render no form and no submit button, which the sandbox blocks', () => {
    const offenders = panelSources().flatMap((path) => {
      const text = readFileSync(path, 'utf8');
      return text
        .split('\n')
        .map((line, index) => ({ line, index }))
        // A comment may name what it avoids.
        .filter(({ line }) => !/^\s*(\/\/|\*|\/\*|<!--)/.test(line))
        .filter(({ line }) => SUBMITS.some((pattern) => pattern.test(line)))
        .map(({ index }) => `${relative(repo, path)}:${index + 1}`);
    });
    expect(offenders).toEqual([]);
  });
});
