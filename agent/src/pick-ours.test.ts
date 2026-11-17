// The pick's check (RB-090-54, `crates/rbrowser/src/capture/pick_ours.js`): whether the node the
// inspect overlay answered for a click is rBrowser's own - a plugin host (`rbrowser-*` or
// `data-rbrowser-plugin`) or anything inside one, through its shadow roots, closed ones included.
// rBrowser calls it with `Runtime.callFunctionOn` on the node, in its isolated world.
import { afterEach, describe, expect, it } from 'vitest';
import source from '../../crates/rbrowser/src/capture/pick_ours.js?raw';

const check = new Function(`return (${source});`)() as (this: Node) => boolean;

function ours(node: Node): boolean {
  return check.call(node);
}

function add(tag: string, parent: Element | ShadowRoot = document.body): HTMLElement {
  const element = document.createElement(tag);
  parent.append(element);
  return element;
}

afterEach(() => {
  document.body.innerHTML = '';
  for (const node of [...document.documentElement.children]) {
    if (node !== document.head && node !== document.body) node.remove();
  }
});

describe('the pick check', () => {
  it('knows the layer host, a plugin host and what they hold', () => {
    const layers = add('rbrowser-layers', document.documentElement);
    const handle = add('div', layers.attachShadow({ mode: 'closed' }));
    expect(ours(layers)).toBe(true);
    expect(ours(handle)).toBe(true);
    const plugin = add('div', document.documentElement);
    plugin.setAttribute('data-rbrowser-plugin', 'rulers-guides');
    const inner = add('span', plugin.attachShadow({ mode: 'open' }));
    expect(ours(inner)).toBe(true);
    const text = document.createTextNode('label');
    inner.append(text);
    expect(ours(text)).toBe(true);
  });

  it('leaves the page alone, its own shadow roots too', () => {
    const main = add('main');
    const button = add('button', main);
    const widget = add('my-widget', main);
    const inside = add('span', widget.attachShadow({ mode: 'closed' }));
    expect(ours(main)).toBe(false);
    expect(ours(button)).toBe(false);
    expect(ours(inside)).toBe(false);
    expect(ours(document.documentElement)).toBe(false);
  });

  it('does not take a page element that only looks like a host in its name', () => {
    const lookalike = add('div');
    lookalike.className = 'rbrowser-layers';
    lookalike.id = 'rbrowser-rulers';
    expect(ours(lookalike)).toBe(false);
  });
});
