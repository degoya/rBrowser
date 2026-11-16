// "Inspect element"'s hit test (RB-090-51, `crates/rbrowser/src/devtools/inspect_point.js`): the
// topmost element of the page under a point, rBrowser's own hosts passed over. rBrowser calls it
// with `Runtime.callFunctionOn` in its isolated world; happy-dom has no layout, so every
// `elementsFromPoint` here is a stub that answers what Chromium's hit test would.
import { afterEach, describe, expect, it, vi } from 'vitest';
import source from '../../crates/rbrowser/src/devtools/inspect_point.js?raw';

type ElementAt = (x: number, y: number) => Element | null;

const elementAt = new Function(`return (${source});`)() as ElementAt;

type Scope = Document | ShadowRoot;

function hits(scope: Scope, answer: Element[], calls: Array<[number, number]> = []): Array<[number, number]> {
  Object.defineProperty(scope, 'elementsFromPoint', {
    configurable: true,
    value: (x: number, y: number) => {
      calls.push([x, y]);
      return answer;
    },
  });
  return calls;
}

function add(tag: string, parent: Element = document.body): HTMLElement {
  const element = document.createElement(tag);
  parent.append(element);
  return element;
}

/** An <iframe> as the page's: happy-dom would try to load a connected one, so it stays out of
 * the tree and only says it is in the document. */
function pageFrame(): HTMLIFrameElement {
  const frame = document.createElement('iframe');
  Object.defineProperty(frame, 'getRootNode', { configurable: true, value: () => document });
  return frame;
}

afterEach(() => {
  vi.restoreAllMocks();
  document.body.innerHTML = '';
  for (const node of [...document.documentElement.children]) {
    if (node !== document.head && node !== document.body) node.remove();
  }
  Reflect.deleteProperty(document, 'elementsFromPoint');
});

describe('inspect point', () => {
  it('passes over the rulers host and answers the page element under it', () => {
    const rulers = add('rbrowser-rulers-guides', document.documentElement);
    rulers.setAttribute('data-rbrowser-plugin', 'rulers-guides');
    const content = add('main');
    const calls = hits(document, [rulers, content, document.body, document.documentElement]);
    expect(elementAt(120, 40)).toBe(content);
    expect(calls).toEqual([[120, 40]]);
  });

  it('passes over every rBrowser host: an `rbrowser-*` tag, `data-rbrowser-plugin`, and what they hold', () => {
    const peek = add('rbrowser-peek', document.documentElement);
    const hello = add('div', document.documentElement);
    hello.setAttribute('data-rbrowser-plugin', 'hello-panel');
    const slotted = add('svg', add('rbrowser-simulators', document.documentElement));
    const page = add('p');
    hits(document, [peek, hello, slotted, page, document.body]);
    expect(elementAt(1, 1)).toBe(page);
  });

  it("passes over the layers in rBrowser's one host <rbrowser-layers> (RB-090-51)", () => {
    const layers = add('rbrowser-layers', document.documentElement);
    const rulers = add('rbrowser-rulers-guides', layers);
    const hello = add('div', layers);
    hello.setAttribute('data-rbrowser-plugin', 'hello-panel');
    const page = add('p');
    // `display: contents` gives the container no box: the hit test names its children only.
    hits(document, [rulers, hello, page, document.body]);
    expect(elementAt(3, 3)).toBe(page);
  });

  it('answers null when only rBrowser is under the point', () => {
    const host = add('rbrowser-image-overlay', document.documentElement);
    hits(document, [host]);
    expect(elementAt(5, 5)).toBeNull();
  });

  it('keeps a page element whose name merely contains rbrowser', () => {
    const own = add('my-rbrowser-widget');
    hits(document, [own, document.body]);
    expect(elementAt(5, 5)).toBe(own);
  });

  it('goes on into an open shadow root of the page, past an rBrowser host in front', () => {
    const rulers = add('rbrowser-rulers-guides', document.documentElement);
    const component = add('page-card');
    const root = component.attachShadow({ mode: 'open' });
    const button = document.createElement('button');
    root.append(button);
    hits(document, [rulers, component, document.body]);
    hits(root, [button]);
    expect(elementAt(10, 10)).toBe(button);
  });

  it('stays at the shadow host when its shadow root has nothing of its own there', () => {
    const component = add('page-card');
    const root = component.attachShadow({ mode: 'open' });
    hits(document, [component, document.body]);
    hits(root, [document.body, document.documentElement]);
    expect(elementAt(10, 10)).toBe(component);
  });

  it('stays at the host of a closed shadow root: the page keeps it to itself', () => {
    const component = add('page-card');
    component.attachShadow({ mode: 'closed' }).append(document.createElement('span'));
    hits(document, [component, document.body]);
    expect(elementAt(10, 10)).toBe(component);
  });

  it('goes on into a same-origin frame at the point inside its content box', () => {
    const frame = pageFrame();
    // A disconnected element has no computed style in happy-dom: its padding comes from the stub.
    const padding = { paddingLeft: '3px', paddingTop: '3px' } as CSSStyleDeclaration;
    vi.spyOn(window, 'getComputedStyle').mockImplementation((element) => {
      expect(element).toBe(frame);
      return padding;
    });
    const inner = document.implementation.createHTMLDocument('inner');
    const field = inner.createElement('input');
    inner.body.append(field);
    Object.defineProperty(frame, 'contentDocument', { configurable: true, value: inner });
    Object.defineProperty(frame, 'clientLeft', { configurable: true, value: 2 });
    Object.defineProperty(frame, 'clientTop', { configurable: true, value: 1 });
    frame.getBoundingClientRect = () => new DOMRect(100, 50, 300, 200);
    hits(document, [frame, document.body]);
    const calls = hits(inner, [field, inner.body]);
    expect(elementAt(150, 80)).toBe(field);
    expect(calls).toEqual([[150 - 105, 80 - 54]]);
  });

  it('stays at a frame whose document it may not read', () => {
    const frame = pageFrame();
    Object.defineProperty(frame, 'contentDocument', {
      configurable: true,
      get: () => {
        throw new DOMException('Blocked a frame', 'SecurityError');
      },
    });
    hits(document, [frame, document.body]);
    expect(elementAt(150, 80)).toBe(frame);
  });
});
