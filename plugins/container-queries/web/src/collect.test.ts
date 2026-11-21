// The analysis of a page against the fixture with 3 containers and 4 @container rules
// (tests/fixtures/containers.html, RB-094-01's acceptance). happy-dom has no layout, so the sizes
// are the fixture's own, given by the environment; a width rBrowser set inline wins, as it would.
import { beforeEach, describe, expect, it } from 'vitest';
import { loadPage, pluginFile } from '../../../web-common/src/load';
import { analyze, findByPath, pathOf } from './collect';
import { fixtureEnv } from './fixture-env';
import { resizeElement, resetElement } from './resize';

const env = fixtureEnv;

beforeEach(() => {
  loadPage(pluginFile('tests/fixtures/containers.html'));
});

describe('the analysis of a page', () => {
  it('finds the 3 containers with their names, types and sizes', () => {
    const report = analyze(document, env);
    expect(report.containers.map((c) => [c.selector, c.names, c.type, c.inline, c.block])).toEqual([
      ['main.shell', ['shell'], 'inline-size', 1000, 600],
      ['article#hero', ['card'], 'inline-size', 640, 300],
      ['aside.side', [], 'size', 300, 250],
    ]);
    expect(report.unreadableSheets).toBe(0);
    expect(report.truncated).toBe(false);
  });

  it('lists exactly the 4 conditions with their bounds', () => {
    const report = analyze(document, env);
    expect(report.conditions.map((c) => [c.name, c.query, c.bounds.map((b) => `${b.axis} ${b.op} ${b.px}`)])).toEqual([
      ['card', '(min-width: 500px)', ['inline >= 500']],
      ['card', '(max-width: 300px)', ['inline <= 300']],
      ['shell', '(400px <= width < 900px)', ['inline >= 400', 'inline < 900']],
      ['', '(min-height: 200px)', ['block >= 200']],
    ]);
    expect(report.conditions.every((c) => c.supported)).toBe(true);
  });

  it('ties each condition to the container its rule queries and says which match', () => {
    const [shell, card, side] = analyze(document, env).containers;
    expect([shell?.conditions, shell?.matching]).toEqual([[2], []]);
    expect([card?.conditions, card?.matching]).toEqual([[0, 1], [0]]);
    expect([side?.conditions, side?.matching]).toEqual([[3], [3]]);
    expect(card?.thresholds).toEqual({ inline: [300, 500], block: [] });
    expect(shell?.thresholds).toEqual({ inline: [400, 900], block: [] });
  });

  it('sees a container dragged to 400 px: (min-width: 500px) no longer matches, and a reset undoes it', () => {
    const hero = document.getElementById('hero') as HTMLElement;
    const before = hero.style.cssText;
    resizeElement(hero, 400);
    const card = analyze(document, env).containers[1];
    expect(card?.inline).toBe(400);
    expect(card?.matching).toEqual([]);
    expect(card?.resized).toBe(true);
    expect(resetElement(hero)).toBe(true);
    expect(hero.style.cssText).toBe(before);
    expect(analyze(document, env).containers[1]?.matching).toEqual([0]);
  });

  it('finds an element again by its path (the shared form of the findings) and by any selector', () => {
    for (const element of document.querySelectorAll('.shell, .card, .side')) {
      expect(findByPath(document, pathOf(element))).toBe(element);
    }
    // No id, no classes (RB-094-10): the same path as every other plugin names the element by.
    expect(pathOf(document.getElementById('hero') as Element)).toBe('html > body > main > article');
    expect(findByPath(document, '#hero')).toBe(document.getElementById('hero'));
  });

  it('skips a rule inside an @media that does not apply and counts unreadable sheets', () => {
    const style = document.createElement('style');
    style.textContent = '@media print { @container card (min-width: 10px) { .card .title { color: red } } }';
    document.head.append(style);
    const report = analyze(document, { ...env, media: (query) => query !== 'print' });
    expect(report.conditions).toHaveLength(5);
    expect(report.containers[1]?.conditions).toEqual([0, 1]);
  });

  it('keeps out what the exclude element holds and bounds the lists', () => {
    const report = analyze(document, env, { exclude: document.querySelector('main'), maxContainers: 5 });
    expect(report.containers).toEqual([]);
    const cut = analyze(document, env, { maxContainers: 2, maxConditions: 3 });
    expect(cut.containers).toHaveLength(2);
    expect(cut.conditions).toHaveLength(3);
    expect(cut.truncated).toBe(true);
  });
});
