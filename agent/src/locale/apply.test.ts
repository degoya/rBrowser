import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import fixture from '../../../tests/fixtures/site/pseudo-locale.html?raw';
import { loadPage } from '../test-page';
import { PageLocaleController } from './apply';
import { originalText } from './originals';

const pseudo = { factor: 1.4, accents: true, rtl: false };

/** Lets the mutation observer run. */
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

/** The page's text nodes the pseudo-localisation may change, in document order (not those of
 *  script, style, code, pre and editable content). */
function texts(): Text[] {
  const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
  const found: Text[] = [];
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    const parent = node.parentElement;
    const kept = parent?.closest('script,style,code,pre,[contenteditable]');
    if (node instanceof Text && node.data.trim() && parent && !kept) {
      found.push(node);
    }
  }
  return found;
}

let controller: PageLocaleController;

beforeEach(() => {
  document.documentElement.setAttribute('lang', 'en');
  document.documentElement.removeAttribute('dir');
  loadPage(fixture);
  controller = new PageLocaleController(window);
});

afterEach(() => {
  controller.stop();
});

describe('dir and lang', () => {
  it('are forced on <html> live and given back as the page had them', () => {
    controller.apply({ dir: 'rtl', lang: 'ar' });
    expect(document.documentElement.dir).toBe('rtl');
    expect(document.documentElement.lang).toBe('ar');
    controller.apply({ dir: 'ltr' });
    expect(document.documentElement.dir).toBe('ltr');
    expect(document.documentElement.lang).toBe('en');
    controller.apply({});
    expect(document.documentElement.hasAttribute('dir')).toBe(false);
    expect(document.documentElement.lang).toBe('en');
  });

  it('refuse a lang that is no BCP 47 tag', () => {
    controller.apply({ lang: 'ar" onload="x' });
    expect(document.documentElement.lang).toBe('en');
  });
});

describe('the pseudo-localisation', () => {
  it('makes each of the fixture\'s ten texts at least 30 % longer and leaves the rest alone', () => {
    const before = texts().map((node) => node.data.trim());
    expect(before).toHaveLength(10);
    const input = document.querySelector<HTMLInputElement>('#name');
    const script = document.querySelector('script')?.textContent;
    const style = document.querySelector('style')?.textContent;
    const code = document.querySelector('code')?.textContent;
    const editable = document.querySelector('[contenteditable]')?.textContent;
    controller.apply({ pseudo });
    const after = texts();
    expect(after).toHaveLength(10);
    after.forEach((node, index) => {
      const original = before[index] ?? '';
      expect(node.data.trim().length, node.data).toBeGreaterThanOrEqual(Math.ceil(original.length * 1.3));
    });
    expect(input?.value).toBe('Ada');
    expect(document.querySelector('script')?.textContent).toBe(script);
    expect(document.querySelector('style')?.textContent).toBe(style);
    expect(document.querySelector('code')?.textContent).toBe(code);
    expect(document.querySelector('[contenteditable]')?.textContent).toBe(editable);
  });

  it('is taken back without a reload', () => {
    const before = document.body.innerHTML;
    controller.apply({ pseudo });
    expect(document.body.innerHTML).not.toBe(before);
    controller.apply({});
    expect(document.body.innerHTML).toBe(before);
  });

  it('keeps the original texts for the selectors', () => {
    controller.apply({ pseudo });
    const button = document.querySelector('#save');
    expect(button?.textContent).not.toBe('Save');
    expect(originalText(button)).toBe('Save');
  });

  it('follows texts the page adds or changes later', async () => {
    controller.apply({ pseudo });
    const added = document.createElement('p');
    added.textContent = 'Added later';
    document.body.append(added);
    await settle();
    expect(added.textContent).not.toBe('Added later');
    expect(originalText(added)).toBe('Added later');
    const first = added.firstChild as Text;
    first.data = 'Changed by the page';
    await settle();
    expect(first.data).not.toBe('Changed by the page');
    expect(originalText(added)).toBe('Changed by the page');
  });

  it('applies new options over the original, not over the pseudo text', () => {
    controller.apply({ pseudo });
    controller.apply({ pseudo: { ...pseudo, accents: false, factor: 1 } });
    expect(document.querySelector('#save')?.textContent).toBe('[Save]');
  });
});
