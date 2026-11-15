import { beforeEach, describe, expect, it } from 'vitest';
import { composedPath, composedSelector, tabIndexOf, tabOrder } from './order';

/** The `data-t` names of the stops, in order. */
const names = (doc: Document = document, includeIframes = true): string[] =>
  tabOrder(doc, { includeIframes }).map((stop) => (stop.iframe ? `iframe:${stop.element.getAttribute('data-t')}` : (stop.element.getAttribute('data-t') ?? stop.element.localName)));

beforeEach(() => {
  document.body.innerHTML = '';
});

describe('the tabindex of an element', () => {
  it('is its attribute, 0 for a natively focusable element, none for the rest', () => {
    document.body.innerHTML = `
      <a data-t="link" href="#x">x</a><a data-t="anchor">no href</a><button data-t="button">b</button>
      <input data-t="hidden" type="hidden"><div data-t="div">d</div><div data-t="three" tabindex="3">t</div>
      <span data-t="minus" tabindex="-1">m</span><span data-t="junk" tabindex="x">j</span>
      <button data-t="disabled" disabled tabindex="2">d</button><div data-t="edit" contenteditable="true">e</div>
      <details><summary data-t="summary">s</summary><summary data-t="second">s2</summary></details>`;
    const index = (name: string) => tabIndexOf(document.querySelector(`[data-t="${name}"]`) as Element);
    expect(index('link')).toBe(0);
    expect(index('anchor')).toBeUndefined();
    expect(index('button')).toBe(0);
    expect(index('hidden')).toBeUndefined();
    expect(index('div')).toBeUndefined();
    expect(index('three')).toBe(3);
    expect(index('minus')).toBe(-1);
    expect(index('junk')).toBeUndefined();
    expect(index('disabled')).toBeUndefined();
    expect(index('edit')).toBe(0);
    expect(index('summary')).toBe(0);
    expect(index('second')).toBeUndefined();
  });
});

describe('the tab order', () => {
  it('puts positive tabindex first, ascending and stable, then the rest in tree order', () => {
    document.body.innerHTML = `
      <a data-t="a" href="#a">a</a>
      <button data-t="five" tabindex="5">5</button>
      <div><input data-t="input"></div>
      <button data-t="three" tabindex="3">3</button>
      <button data-t="three-b" tabindex="3">3b</button>
      <span data-t="minus" tabindex="-1">m</span>
      <textarea data-t="text"></textarea>`;
    expect(names()).toEqual(['three', 'three-b', 'five', 'a', 'input', 'text']);
  });

  it('leaves out disabled, inert, display:none, visibility:hidden and the overlay host', () => {
    document.body.innerHTML = `
      <button data-t="one">1</button>
      <button data-t="disabled" disabled>d</button>
      <fieldset disabled><input data-t="in-fieldset"></fieldset>
      <section inert><a data-t="inert" href="#i">i</a></section>
      <div style="display:none"><a data-t="none" href="#n">n</a></div>
      <a data-t="hidden" href="#h" style="visibility:hidden">h</a>
      <div style="visibility:hidden"><a data-t="shown-again" href="#s" style="visibility:visible">s</a></div>
      <rbrowser-overlays><button data-t="own">o</button></rbrowser-overlays>
      <button data-t="two">2</button>`;
    expect(names()).toEqual(['one', 'shown-again', 'two']);
  });

  it('takes one radio button per group: the checked one, else the first', () => {
    document.body.innerHTML = `
      <input data-t="r1" type="radio" name="size"><input data-t="r2" type="radio" name="size" checked>
      <input data-t="c1" type="radio" name="colour"><input data-t="c2" type="radio" name="colour">
      <input data-t="lone" type="radio">
      <form><input data-t="f1" type="radio" name="size"></form>
      <form><input data-t="f2" type="radio" name="size"></form>`;
    expect(names()).toEqual(['r2', 'c1', 'lone', 'f1', 'f2']);
  });

  it('places the elements of an open shadow root where its host stands, slotted content at its slot', () => {
    document.body.innerHTML = `
      <button data-t="before">b</button>
      <x-card data-t="card"><a data-t="slotted" href="#s" slot="action">s</a></x-card>
      <button data-t="after">a</button>`;
    const host = document.querySelector('x-card') as HTMLElement;
    const root = host.attachShadow({ mode: 'open' });
    root.innerHTML = '<button data-t="inner">i</button><slot name="action"></slot><input data-t="last">';
    expect(names()).toEqual(['before', 'inner', 'slotted', 'last', 'after']);
  });

  it('orders a positive tabindex inside a shadow root within that root, and skips a host with tabindex -1', () => {
    document.body.innerHTML = '<x-a data-t="host-a"></x-a><button data-t="mid">m</button><x-b data-t="host-b" tabindex="-1"></x-b>';
    const a = (document.querySelector('x-a') as HTMLElement).attachShadow({ mode: 'open' });
    a.innerHTML = '<button data-t="a1">1</button><button data-t="a2" tabindex="1">2</button>';
    const b = (document.querySelector('x-b') as HTMLElement).attachShadow({ mode: 'open' });
    b.innerHTML = '<button data-t="b1">1</button>';
    expect(names()).toEqual(['a2', 'a1', 'mid']);
  });

  it('lists a focusable host before its shadow content', () => {
    document.body.innerHTML = '<x-toggle data-t="toggle" tabindex="0"></x-toggle>';
    const root = (document.querySelector('x-toggle') as HTMLElement).attachShadow({ mode: 'open' });
    root.innerHTML = '<a data-t="help" href="#h">?</a>';
    expect(names()).toEqual(['toggle', 'help']);
  });

  it('marks an iframe it does not walk as one stop', () => {
    document.body.innerHTML = '<button data-t="one">1</button><iframe data-t="frame"></iframe><button data-t="two">2</button>';
    expect(names(document, false)).toEqual(['one', 'iframe:frame', 'two']);
  });
});

describe('the composed selector and path', () => {
  it('join the host and the element in its shadow root with >>>', () => {
    document.body.innerHTML = '<main><x-card id="card"></x-card></main>';
    const root = (document.querySelector('x-card') as HTMLElement).attachShadow({ mode: 'open' });
    root.innerHTML = '<div><button class="buy">b</button></div>';
    const button = root.querySelector('button') as Element;
    expect(composedSelector(button)).toBe('#card >>> button.buy');
    expect(composedPath(button)).toBe('html > body > main > x-card >>> div > button');
  });
});
