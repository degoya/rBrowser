import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';

import { bodyOf, collect, fixturePage, loadModel, loadPage, loadPanel, panelFile, pluginFile, runScript, writePluginFile } from './load';

// A plugin folder of its own, with this test's cwd as its `web/`: the loader reads the parent.
let plugin: string;
let cwd: string;

beforeAll(() => {
  plugin = mkdtempSync(join(tmpdir(), 'rb-web-common-'));
  writeFileSync(join(plugin, 'panel.html'), '<!doctype html><title>x</title><style>b{color:red}</style><body><b id="t">T</b><script>bad()</script></body>');
  writeFileSync(join(plugin, 'model.js'), 'var TestModel = { answer: 42 };');
  writeFileSync(join(plugin, 'collect.js'), '(function (options) { return { got: options.x * 2 }; })');
  cwd = process.cwd();
  process.chdir(join(plugin));
});

afterAll(() => {
  process.chdir(cwd);
  rmSync(plugin, { recursive: true, force: true });
});

describe('the loader', () => {
  it('reads files of the plugin and writes them back', () => {
    // `pluginFile` resolves against the parent of the cwd; the temp folder is that parent's child.
    const name = `${plugin.split('/').pop()}/model.js`;
    expect(pluginFile(name)).toContain('TestModel');
    writePluginFile(`${plugin.split('/').pop()}/written.txt`, 'hi');
    expect(pluginFile(`${plugin.split('/').pop()}/written.txt`)).toBe('hi');
  });

  it('takes a body without its scripts and puts a page into the document', () => {
    const html = pluginFile(`${plugin.split('/').pop()}/panel.html`);
    expect(bodyOf(html)).toBe('<b id="t">T</b>');
    loadPage(html);
    expect(document.head.innerHTML).toContain('b{color:red}');
    expect(document.getElementById('t')?.textContent).toBe('T');
  });

  it('runs a classic script in the global scope and answers its model', () => {
    const dir = plugin.split('/').pop();
    runScript(`${dir}/model.js`);
    expect((globalThis as Record<string, unknown>).TestModel).toEqual({ answer: 42 });
    const model = loadModel(() => (globalThis as unknown as { TestModel: { answer: number } }).TestModel, `${dir}/model.js`);
    expect(model.answer).toBe(42);
  });

  it('runs a one-function script with its options and loads a panel with its scripts', () => {
    const dir = plugin.split('/').pop();
    expect(collect(`${dir}/collect.js`, { x: 21 })).toEqual({ got: 42 });
    loadPanel([`${dir}/model.js`], `${dir}/panel.html`);
    expect(document.body.innerHTML).toBe('<b id="t">T</b>');
  });

  it('names the panel folder and the fixture site', () => {
    expect(() => panelFile('missing.js')).toThrow(/panel/);
    expect(() => fixturePage('missing/index.html')).toThrow(/fixtures/);
  });
});
