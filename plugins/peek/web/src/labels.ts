// The tooltip's texts: the plugin's own catalogue (plugins/peek/locales/en.json), bundled into
// the page script - a page script gets no texts from the host. English only in phase 1.

import catalogue from '../../locales/en.json';

type Tree = { [key: string]: string | Tree };

const ROOT = (catalogue as { plugin: { peek: Tree } }).plugin.peek;

/** The text of `plugin.peek.<key>` with `{name}` placeholders filled; the key when unknown. */
export function t(key: string, params: Record<string, string | number> = {}): string {
  let node: string | Tree | undefined = ROOT;
  for (const part of key.split('.')) {
    node = typeof node === 'object' ? node[part] : undefined;
  }
  if (typeof node !== 'string') return key;
  return node.replace(/\{(\w+)\}/g, (whole, name: string) =>
    name in params ? String(params[name]) : whole,
  );
}
