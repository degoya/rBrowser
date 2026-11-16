import { afterEach, describe, expect, it } from 'vitest';
import { inspect } from './collect-entry';
import { collect, elementAt, MAX_LAYERS, MAX_TEXT } from './collect';

afterEach(() => {
  document.body.replaceChildren();
  document.body.removeAttribute('style');
});

function fixture(): HTMLButtonElement {
  document.body.style.backgroundColor = 'rgb(255, 255, 255)';
  const wrapper = document.createElement('div');
  wrapper.style.backgroundColor = 'rgba(0, 0, 0, 0.5)';
  const button = document.createElement('button');
  button.id = 'send';
  button.className = 'btn btn-primary';
  button.textContent = 'Send';
  button.style.cssText =
    'color: rgb(119, 119, 119); font-size: 16px; font-weight: 700; margin: 4px 8px; padding: 6px 12px; border: 1px solid rgb(0, 0, 0)';
  wrapper.append(button);
  document.body.append(wrapper);
  return button;
}

describe('the collector', () => {
  it('reads tag, classes, colours, font, box, role, name and the selector', () => {
    const data = collect(fixture(), { x: 3, y: 4 });
    expect(data.tag).toBe('button');
    expect(data.id).toBe('send');
    expect(data.classes).toEqual(['btn', 'btn-primary']);
    expect(data.color).toBe('rgb(119, 119, 119)');
    expect(data.font.size).toBe(16);
    expect(data.font.weight).toBe(700);
    expect(data.box.margin).toEqual([4, 8, 4, 8]);
    expect(data.box.padding).toEqual([6, 12, 6, 12]);
    expect(data.box.border).toEqual([1, 1, 1, 1]);
    expect(data.role).toBe('button');
    expect(data.name).toBe('Send');
    expect(data.selector).toBe('#send');
    expect(data.point).toEqual({ x: 3, y: 4 });
    expect(data.target?.element).toMatchObject({ kind: 'role', role: 'button', name: 'Send' });
  });

  // RB-090-54: `DOM.getNodeForLocation` takes document px, the pointer is in the viewport's - the
  // page's scroll goes with the point, so the component finds the element on a scrolled page.
  it('sends the scroll of the page with the point', () => {
    const scrolled = { scrollX: 30, scrollY: 500 };
    for (const [key, value] of Object.entries(scrolled)) {
      Object.defineProperty(window, key, { value, configurable: true });
    }
    try {
      const data = collect(fixture(), { x: 3, y: 4 });
      expect(data.point).toEqual({ x: 3, y: 4 });
      expect(data.scroll).toEqual({ x: 30, y: 500 });
    } finally {
      for (const key of Object.keys(scrolled)) {
        Reflect.deleteProperty(window, key);
      }
    }
    expect(collect(fixture()).scroll).toEqual({ x: 0, y: 0 });
  });

  it('walks the backgrounds outward and stops at the first opaque one', () => {
    const data = collect(fixture());
    const colors = data.backgrounds.map((layer) => layer.color);
    expect(colors.slice(1, 3)).toEqual(['rgba(0, 0, 0, 0.5)', 'rgb(255, 255, 255)']);
    expect(colors).toHaveLength(3);
    expect(data.backgrounds.every((layer) => layer.image === false)).toBe(true);
  });

  it('marks a background image', () => {
    const button = fixture();
    button.style.backgroundImage = 'linear-gradient(red, blue)';
    expect(collect(button).backgrounds[0]?.image).toBe(true);
  });

  it('reads at most MAX_LAYERS ancestors', () => {
    let parent: HTMLElement = document.body;
    for (let index = 0; index < MAX_LAYERS + 10; index += 1) {
      const child = document.createElement('div');
      parent.append(child);
      parent = child;
    }
    document.body.style.backgroundColor = '';
    expect(collect(parent).backgrounds.length).toBe(MAX_LAYERS);
  });

  it('has no selector inside a shadow root but a target through its host', () => {
    const host = document.createElement('div');
    host.id = 'host';
    document.body.append(host);
    const inner = document.createElement('span');
    inner.textContent = 'inside';
    host.attachShadow({ mode: 'open' }).append(inner);
    const data = collect(inner);
    expect(data.selector).toBeNull();
    expect(data.target?.hosts).toHaveLength(1);
  });

  it('cuts what the page makes long and leaves out an oversized selector and target', () => {
    const button = fixture();
    button.className = `${'k'.repeat(5000)} other`;
    button.id = 'i'.repeat(5000);
    let parent: HTMLElement = document.body;
    for (let index = 0; index < 300; index += 1) {
      const child = document.createElement('section');
      child.className = `level-${index}`;
      parent.append(child, document.createElement('section'));
      parent = child;
    }
    const deep = document.createElement('span');
    parent.append(deep);
    const data = collect(deep);
    expect(data.selector).toBeNull();
    expect(data.target).toBeNull();
    const long = collect(button);
    expect(long.id.length).toBe(MAX_TEXT);
    expect(long.classes[0]?.length).toBe(MAX_TEXT);
    expect(JSON.stringify(long).length).toBeLessThan(64 * 1024);
  });

  it('the MCP collector finds by selector, and answers null for nothing or a bad selector', () => {
    fixture();
    expect(inspect({ selector: '#send' })?.tag).toBe('button');
    expect(inspect({ selector: '#gone' })).toBeNull();
    expect(inspect({ selector: '##bad((' })).toBeNull();
    expect(inspect({})).toBeNull();
  });

  it('elementAt goes through open shadow roots', () => {
    const outer = document.createElement('div');
    const inner = document.createElement('p');
    outer.attachShadow({ mode: 'open' }).append(inner);
    const doc = { elementFromPoint: () => outer } as unknown as Document;
    Object.defineProperty(outer.shadowRoot, 'elementFromPoint', { value: () => inner });
    expect(elementAt(doc, 1, 2)).toBe(inner);
  });
});
