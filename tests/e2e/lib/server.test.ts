import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import { startFixtureServer } from './server.ts';
import type { FixtureServer } from './server.ts';

let root: string;
let other: string;
let server: FixtureServer;

beforeEach(async () => {
  root = mkdtempSync(join(tmpdir(), 'e2e-site-'));
  other = mkdtempSync(join(tmpdir(), 'e2e-mount-'));
  writeFileSync(join(root, 'form.html'), '<!doctype html><title>Form</title>');
  mkdirSync(join(root, 'dir'));
  writeFileSync(join(root, 'dir', 'index.html'), 'index');
  writeFileSync(join(root, 'robots.txt'), 'site robots');
  writeFileSync(join(other, 'page.html'), '<a href="https://fixture.test/x/">x</a>');
  writeFileSync(join(other, 'robots.txt'), 'Sitemap: https://fixture.test/x/sitemap.xml');
  writeFileSync(
    join(other, 'headers.json'),
    JSON.stringify({ 'en/': { link: '<https://fixture.test/x/en/>; rel="alternate"' } }),
  );
  mkdirSync(join(other, 'en'));
  writeFileSync(join(other, 'en', 'index.html'), 'en');
  server = await startFixtureServer({
    root,
    ports: [0],
    mounts: [{ prefix: '/x/', dir: other, headersFile: 'headers.json' }],
    rewriteOrigin: 'https://fixture.test',
  });
});

afterEach(async () => {
  await server.close();
});

describe('the fixture server', () => {
  it('serves the site with a content type, a folder by its index.html', async () => {
    const page = await fetch(`${server.origin}/form.html`);
    expect(page.status).toBe(200);
    expect(page.headers.get('content-type')).toBe('text/html; charset=utf-8');
    expect(await (await fetch(`${server.origin}/dir/`)).text()).toBe('index');
  });

  it('answers 404 for a missing file and never leaves its folder', async () => {
    expect((await fetch(`${server.origin}/missing.html`)).status).toBe(404);
    expect((await fetch(`${server.origin}/..%2f..%2fetc%2fpasswd`)).status).toBe(404);
    expect((await fetch(`${server.origin}/x/..%2f..%2fform.html`)).status).toBe(404);
  });

  it('counts the form posts, redirects to the form and resets', async () => {
    const post = await fetch(`${server.origin}/submit`, { method: 'POST', body: 'name=a', redirect: 'manual' });
    expect(post.status).toBe(303);
    expect(post.headers.get('location')).toBe('/form.html?posts=1');
    expect(await (await fetch(`${server.origin}/count`)).json()).toEqual({ posts: 1 });
    expect(server.posts()).toBe(1);
    server.reset();
    expect(server.posts()).toBe(0);
  });

  it('shows a GET search query escaped', async () => {
    const page = await (await fetch(`${server.origin}/search?q=%3Cb%3E`)).text();
    expect(page).toContain('Results for &lt;b&gt;');
  });

  it('stamps what pages report to /log in its own clock', async () => {
    const before = Date.now();
    const answer = await fetch(`${server.origin}/log`, {
      method: 'POST',
      body: JSON.stringify({ marker: 'css-1', view: 'mobile' }),
    });
    expect(answer.status).toBe(204);
    const [entry] = server.reports();
    expect(entry).toMatchObject({ marker: 'css-1', view: 'mobile' });
    expect(entry?.at).toBeGreaterThanOrEqual(before);
  });

  it('logs every request with its method, path and the headers asked about', async () => {
    await fetch(`${server.origin}/form.html`, { headers: { 'X-Untrusted': 'project' } });
    const seen = server.requests().find((request) => request.path === '/form.html');
    expect(seen?.method).toBe('GET');
    expect(seen?.headers['x-untrusted']).toBe('project');
  });

  it('serves a mount with its headers file and the fixture origin rewritten to its own', async () => {
    const page = await (await fetch(`${server.origin}/x/page.html`)).text();
    expect(page).toBe(`<a href="${server.origin}/x/">x</a>`);
    const english = await fetch(`${server.origin}/x/en/`);
    expect(english.headers.get('link')).toBe(`<${server.origin}/x/en/>; rel="alternate"`);
    expect((await fetch(`${server.origin}/x/headers.json`)).status).toBe(404);
  });

  it('serves a file of a mount at another path while asked to', async () => {
    expect(await (await fetch(`${server.origin}/robots.txt`)).text()).toBe('site robots');
    server.alias('/robots.txt', join(other, 'robots.txt'));
    expect(await (await fetch(`${server.origin}/robots.txt`)).text()).toBe(
      `Sitemap: ${server.origin}/x/sitemap.xml`,
    );
    server.alias('/robots.txt', null);
    expect(await (await fetch(`${server.origin}/robots.txt`)).text()).toBe('site robots');
  });
});
