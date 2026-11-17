// axe-core itself (the pinned node_modules copy, the same file as dist/axe.min.js) against the
// fixture page tests/fixtures/site/a11y.html, through the runner: the nameless menu button is a
// finding only below 768 px, the image without alt at every width.
import axe from 'axe-core';
import { beforeEach, describe, expect, it } from 'vitest';
import fixture from '../../../../tests/fixtures/site/a11y.html?raw';
import { type AxeLike, type RunOptions, runAxe } from './run';

const OPTIONS: RunOptions = {
  rules: ['button-name', 'image-alt', 'html-has-lang', 'link-name'],
  include: [],
  exclude: [],
  incomplete: false,
  maxNodes: 50,
  maxTotal: 500,
  maxText: 300,
};

type HappyWindow = { happyDOM: { setViewport(size: { width: number; height: number }): void } };

function load(width: number): void {
  (window as unknown as HappyWindow).happyDOM.setViewport({ width, height: 800 });
  const parsed = new DOMParser().parseFromString(fixture, 'text/html');
  document.documentElement.setAttribute('lang', parsed.documentElement.getAttribute('lang') ?? '');
  document.head.innerHTML = parsed.head.innerHTML;
  document.body.innerHTML = parsed.body.innerHTML;
}

async function violations(width: number): Promise<Record<string, string[]>> {
  load(width);
  const answer = await runAxe(axe as unknown as AxeLike, OPTIONS, document);
  return Object.fromEntries(
    answer.violations.map((rule) => [rule.id, rule.nodes.map((node) => node.path)]),
  );
}

describe('axe-core on the a11y fixture', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  it('is the pinned version', () => {
    expect(axe.version).toBe('4.14.0');
  });

  it('finds the nameless menu button on a narrow view', async () => {
    const found = await violations(375);
    expect(found['button-name']).toEqual(['html > body > header > button']);
    expect(found['image-alt']).toEqual(['html > body > main > img']);
  });

  it('does not find the hidden menu button on a wide view', async () => {
    const found = await violations(1280);
    expect(found['button-name']).toBeUndefined();
    expect(found['image-alt']).toEqual(['html > body > main > img']);
  });
});
