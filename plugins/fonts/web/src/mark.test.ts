import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { clear, HOST, mark, type MarkEnv, matches } from './mark';
import { mark as entryMark, clear as entryClear } from './collect-entry';

const scrolled: Element[] = [];
const env: MarkEnv = {
  rect: () => [10, 20, 100, 16],
  shown: () => true,
  scrollTo: (element) => scrolled.push(element),
};

describe('mark', () => {
  beforeEach(() => {
    document.head.innerHTML = '<style>body { font-family: Inter, Arial; } .r { font-family: "Roboto", Arial; } .b { font-weight: 600; }</style>';
    document.body.innerHTML = '<h1 class="b">Title</h1><p>One</p><p class="r">Two</p><div><span>Three</span></div>';
    scrolled.length = 0;
  });
  afterEach(() => {
    clear();
    document.head.innerHTML = '';
    document.body.innerHTML = '';
  });

  it('outlines every element set in the family inside one rbrowser-fonts host', () => {
    expect(mark({ family: 'inter' }, env)).toEqual({ count: 3 });
    const hosts = document.querySelectorAll(HOST);
    expect(hosts).toHaveLength(1);
    expect(HOST.startsWith('rbrowser-')).toBe(true);
    // The shadow root is closed: the page cannot reach the boxes.
    expect((hosts[0] as HTMLElement).shadowRoot).toBeNull();
    expect((hosts[0] as HTMLElement).getAttribute('style')).toContain('pointer-events:none');
    // RB-090-51: a layer of rBrowser's one host of page overlays.
    expect(hosts[0]?.parentElement?.localName).toBe('rbrowser-layers');
    expect(hosts[0]?.getAttribute('data-rbrowser-plugin')).toBe('fonts');
    expect(scrolled.map((element) => element.localName)).toEqual(['h1']);
  });

  it('narrows to one weight and replaces the old marks', () => {
    mark({ family: 'Inter' }, env);
    expect(mark({ family: 'Inter', weight: 600 }, env)).toEqual({ count: 1 });
    expect(document.querySelectorAll(HOST)).toHaveLength(1);
  });

  it('never counts its own host and draws nothing for an unknown family', () => {
    mark({ family: 'Roboto' }, env);
    expect(matches({ family: 'Roboto' }, env)).toHaveLength(1);
    expect(mark({ family: 'Nope' }, env)).toEqual({ count: 0 });
    expect(document.querySelectorAll(HOST)).toHaveLength(0);
    expect(mark({}, env)).toEqual({ count: 0 });
  });

  it('clear removes the marks; the entry takes a malformed request as nothing', () => {
    mark({ family: 'Inter' }, env);
    entryClear();
    expect(document.querySelectorAll(HOST)).toHaveLength(0);
    expect(document.querySelector('rbrowser-layers')).toBeNull();
    expect(entryMark('nonsense')).toEqual({ count: 0 });
  });
});
