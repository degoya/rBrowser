import { beforeEach, describe, expect, it } from 'vitest';
import { type AxeLike, type AxeRule, type RunOptions, contextOf, runAxe, selectorOf } from './run';
import { start } from './main';

const OPTIONS: RunOptions = {
  rules: ['image-alt', 'button-name'],
  include: [],
  exclude: [],
  incomplete: false,
  maxNodes: 2,
  maxTotal: 3,
  maxText: 10,
};

function fakeAxe(violations: AxeRule[], incomplete: AxeRule[] = []) {
  const calls: { context: unknown; options: Record<string, unknown> }[] = [];
  const axe: AxeLike = {
    version: '4.14.1',
    async run(context, options) {
      calls.push({ context, options });
      return { url: 'http://localhost/a11y.html', violations, incomplete };
    },
  };
  return { axe, calls };
}

describe('runAxe', () => {
  beforeEach(() => {
    document.body.innerHTML = '<main><img src="a"><img src="b"><img src="c"></main>';
  });

  it('runs only the rules asked for, without frames, with element references', async () => {
    const { axe, calls } = fakeAxe([]);
    await runAxe(axe, OPTIONS, document);
    expect(calls).toHaveLength(1);
    expect(calls[0]?.context).toBe(document);
    expect(calls[0]?.options).toMatchObject({
      runOnly: { type: 'rule', values: ['image-alt', 'button-name'] },
      resultTypes: ['violations'],
      elementRef: true,
      iframes: false,
    });
  });

  it('runs nothing when no rule is asked for', async () => {
    const { axe, calls } = fakeAxe([]);
    const answer = await runAxe(axe, { ...OPTIONS, rules: [] }, document);
    expect(calls).toHaveLength(0);
    expect(answer.violations).toEqual([]);
  });

  it('shapes nodes with selector, element path and cut texts', async () => {
    const images = Array.from(document.querySelectorAll('img'));
    const { axe } = fakeAxe([
      {
        id: 'image-alt',
        impact: 'critical',
        help: 'Images must have alternative text',
        helpUrl: 'https://dequeuniversity.com/rules/axe/4.14/image-alt',
        tags: ['wcag2a', 'wcag111'],
        nodes: images.map((element, index) => ({
          target: [`main > img:nth-child(${index + 1})`],
          impact: 'critical',
          html: '<img src="a-very-long-name.png">',
          failureSummary: 'Fix any of the following',
          element,
        })),
      },
    ]);
    const answer = await runAxe(axe, OPTIONS, document);
    const rule = answer.violations[0];
    expect(rule?.nodes).toHaveLength(2);
    expect(rule?.omitted).toBe(1);
    expect(answer.truncated).toBe(true);
    expect(rule?.nodes[1]).toEqual({
      selector: 'main > img:nth-child(2)',
      path: 'html > body > main > img:nth-of-type(2)',
      impact: 'critical',
      html: '<img src="...',
      summary: 'Fix any of...',
    });
  });

  it('holds the total node budget across rules and leaves incomplete out unless asked', async () => {
    const node = (index: number) => ({ target: [`#n${index}`] });
    const rule = (id: string) => ({ id, nodes: [node(1), node(2)] });
    const { axe } = fakeAxe([rule('a'), rule('b')], [rule('c')]);
    const without = await runAxe(axe, OPTIONS, document);
    expect(without.violations.map((r) => r.nodes.length)).toEqual([2, 1]);
    expect(without.incomplete).toEqual([]);
    const withIncomplete = await runAxe(axe, { ...OPTIONS, incomplete: true, maxTotal: 10 }, document);
    expect(withIncomplete.incomplete[0]?.nodes).toHaveLength(2);
  });
});

describe('contextOf and selectorOf', () => {
  it('names include and exclude only when given', () => {
    expect(contextOf({ ...OPTIONS, exclude: ['#ads'] }, document)).toEqual({ exclude: ['#ads'] });
    expect(contextOf({ ...OPTIONS, include: ['main'] }, document)).toEqual({ include: ['main'] });
  });

  it('joins a shadow-DOM target with " >>> "', () => {
    expect(selectorOf(['#a'])).toBe('#a');
    expect(selectorOf([['#host', 'button']])).toBe('#host >>> button');
  });
});

describe('start', () => {
  it('removes window.axe after the run, also when axe throws', async () => {
    const global = globalThis as unknown as { axe?: AxeLike };
    global.axe = {
      version: '4.14.1',
      run: () => Promise.reject(new Error("'##' is not a valid selector")),
    };
    const answer = await start(OPTIONS);
    expect(answer).toEqual({ error: "'##' is not a valid selector" });
    expect('axe' in globalThis).toBe(false);
  });

  it('answers an error when axe did not load', async () => {
    expect(await start(OPTIONS)).toEqual({ error: 'axe-core did not load' });
  });
});
