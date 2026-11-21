// The review condition of RB-092-02: a pseudo-localised view keeps the sync. The selectors read
// the original texts, so a target made in one view resolves in another whichever of them shows
// pseudo text - here the same document plays both views, before and after.

import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import fixture from '../../../tests/fixtures/site/pseudo-locale.html?raw';
import { locate } from '../resolve';
import { makeSelector, makeTarget } from '../selector';
import { giveBoxes, loadPage } from '../test-page';
import { PageLocaleController } from './apply';

const pseudo = { factor: 1.4, accents: true, rtl: false };
let controller: PageLocaleController;

beforeEach(() => {
  loadPage(fixture);
  giveBoxes();
  controller = new PageLocaleController(window);
});

afterEach(() => {
  controller.stop();
});

function save(): Element {
  const button = document.querySelector('#save');
  if (!button) throw new Error('no Save button');
  return button;
}

describe('the selectors of a pseudo-localised page', () => {
  it('name a button by its original text', () => {
    controller.apply({ pseudo });
    expect(makeSelector(save())).toMatchObject({ kind: 'role', role: 'button', name: 'Save' });
    expect(makeSelector(document.querySelector('h1') as Element)).toMatchObject({
      kind: 'role',
      name: 'Pseudo-localisation fixture',
    });
  });

  it('find the element whichever view shows pseudo text: 20 clicks, 0 misses', () => {
    for (let click = 0; click < 20; click += 1) {
      const fromPseudo = click % 2 === 0;
      if (fromPseudo) controller.apply({ pseudo });
      const target = makeTarget(save(), 20, 30);
      // The other view.
      controller.apply(fromPseudo ? {} : { pseudo: { ...pseudo, rtl: click % 4 === 1 } });
      expect(locate(target, document)).toBe(save());
      controller.apply({});
    }
  });

  it('match a text selector and a label name against the originals', () => {
    controller.apply({ pseudo });
    const strong = document.querySelector('strong') as Element;
    expect(makeSelector(strong)).toMatchObject({ kind: 'text', text: 'bold words' });
    expect(makeSelector(document.querySelector('#name') as Element)).toMatchObject({
      kind: 'role',
      role: 'textbox',
      name: 'Your name',
    });
  });
});
