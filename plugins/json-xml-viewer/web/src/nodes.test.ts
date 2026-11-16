import { describe, expect, it } from 'vitest';
import { JsonNode, jsonPathStep } from './nodes';
import { xmlRoots } from './fixtures';
import type { XmlNode } from './xml-nodes';

describe('JSON nodes', () => {
  const root = new JsonNode({ items: [{ id: 1 }, { id: 2, 'odd key': "it's" }], ok: true, none: null });

  it('make their children when asked, once', () => {
    expect(root.childCount()).toBe(3);
    expect(root.children()).toBe(root.children());
    expect(root.children().map((node) => node.name)).toEqual(['items', 'ok', 'none']);
  });

  it('answer JSONPath paths', () => {
    const items = root.children()[0] as JsonNode;
    const second = items.children()[1] as JsonNode;
    expect(root.path()).toBe('$');
    expect(second.children()[0]?.path()).toBe('$.items[1].id');
    expect(second.children()[1]?.path()).toBe("$.items[1]['odd key']");
  });

  it('copy a scalar as text and a container as JSON', () => {
    const items = root.children()[0] as JsonNode;
    const odd = (items.children()[1] as JsonNode).children()[1] as JsonNode;
    expect(odd.value()).toBe("it's");
    expect(odd.preview()).toBe('"it\'s"');
    expect(root.children()[2]?.value()).toBe('null');
    expect(JSON.parse(items.value())).toEqual([{ id: 1 }, { id: 2, 'odd key': "it's" }]);
  });

  it('escape odd keys in a step', () => {
    expect(jsonPathStep('a-b')).toBe("['a-b']");
    expect(jsonPathStep("x'\\y")).toBe("['x\\'\\\\y']");
    expect(jsonPathStep(0)).toBe('[0]');
    expect(jsonPathStep('$id')).toBe('.$id');
  });
});

describe('XML nodes', () => {
  const [urlset] = xmlRoots(
    '<urlset><url><loc>https://a.test/</loc></url><url lang="de"><loc>https://a.test/de</loc>' +
      '<!-- note --></url></urlset>',
  );

  it('leave out whitespace and show short text inline', () => {
    const [root] = xmlRoots('<a>\n  <b>text</b>\n  <c><d/></c>\n</a>');
    expect(root?.children().map((node) => node.name)).toEqual(['b', 'c']);
    expect(root?.children()[0]?.inline()).toBe(true);
    expect(root?.children()[0]?.hasChildren()).toBe(false);
    expect(root?.children()[0]?.preview()).toBe('text');
    expect(root?.children()[1]?.hasChildren()).toBe(true);
  });

  it('answer XPath paths with positions only among namesakes', () => {
    const second = urlset?.children()[1] as XmlNode;
    expect(urlset?.path()).toBe('/urlset');
    expect(second.path()).toBe('/urlset/url[2]');
    expect(second.children()[0]?.path()).toBe('/urlset/url[2]/loc');
    expect(second.children()[1]?.path()).toBe('/urlset/url[2]/comment()');
    expect(second.attrs()[0]?.path()).toBe('/urlset/url[2]/@lang');
    expect((second.children()[0] as XmlNode).all()[0]?.path()).toBe('/urlset/url[2]/loc/text()');
  });

  it('copy the text content of an element', () => {
    expect(urlset?.children()[0]?.value()).toBe('https://a.test/');
  });
});
