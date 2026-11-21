import { afterEach, describe, expect, it } from 'vitest';
import { forgetAll, originalOf, originalText, remember } from './originals';

afterEach(() => {
  forgetAll();
  document.body.innerHTML = '';
});

function text(selector: string): Text {
  const node = document.querySelector(selector)?.firstChild;
  if (!(node instanceof Text)) throw new Error(`no text in ${selector}`);
  return node;
}

describe('the original texts', () => {
  it('read the page as it was before the pseudo text', () => {
    document.body.innerHTML = '<button id="b">Save <b>now</b></button>';
    const save = text('#b');
    save.data = '[Š\u00e1ṽ\u00e9] ';
    remember(save, 'Save ', '[Š\u00e1ṽ\u00e9] ');
    expect(originalOf(save)).toBe('Save ');
    expect(originalText(document.querySelector('#b'))).toBe('Save now');
    expect(document.querySelector('#b')?.textContent).toBe('[Š\u00e1ṽ\u00e9] now');
  });

  it('take a text the page wrote over the pseudo text as it is', () => {
    document.body.innerHTML = '<p id="p">Old</p>';
    const node = text('#p');
    remember(node, 'Old', '[\u00d6ļ\u00f0]');
    node.data = 'New by the page';
    expect(originalOf(node)).toBe('New by the page');
  });

  it('skip what a selector excludes, as the label text leaves out its field', () => {
    document.body.innerHTML = '<label id="l">Name <select><option>One</option></select></label>';
    expect(originalText(document.querySelector('#l'), 'input, select, textarea')).toBe('Name ');
  });

  it('are the text content when nothing is pseudo-localised', () => {
    document.body.innerHTML = '<p id="p">Plain <i>text</i><!-- note --></p>';
    expect(originalText(document.querySelector('#p'))).toBe('Plain text');
    expect(originalText(null)).toBe('');
  });
});
