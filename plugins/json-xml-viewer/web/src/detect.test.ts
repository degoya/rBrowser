import { describe, expect, it } from 'vitest';
import { detect, kindOf, XML_SOURCE_ID } from './detect';

function jsonDocument(text: string): Document {
  const doc = document.implementation.createHTMLDocument('');
  const pre = doc.createElement('pre');
  pre.textContent = text;
  doc.body.append(pre);
  return doc;
}

function xmlDocument(text: string): Document {
  return new DOMParser().parseFromString(text, 'application/xml');
}

describe('the kind of a content type', () => {
  it.each([
    ['application/json', 'json'],
    ['text/json', 'json'],
    ['application/ld+json', 'json'],
    ['application/problem+json', 'json'],
    ['APPLICATION/JSON', 'json'],
    ['application/xml', 'xml'],
    ['text/xml', 'xml'],
    ['application/rss+xml', 'xml'],
    ['application/atom+xml', 'xml'],
  ])('%s is %s', (type, kind) => {
    expect(kindOf(type)).toBe(kind);
  });

  it.each(['text/html', 'text/plain', 'image/svg+xml', 'application/xhtml+xml', 'application/javascript', ''])(
    '%s is none',
    (type) => {
      expect(kindOf(type)).toBeNull();
    },
  );
});

describe('detect', () => {
  it('reads a JSON document from the pre Chromium renders it in', () => {
    const found = detect(jsonDocument('{"a": 1}'), 'application/json');
    expect(found).toEqual({ kind: 'json', text: '{"a": 1}' });
  });

  it('leaves a JSON document of another shape alone', () => {
    const doc = document.implementation.createHTMLDocument('');
    doc.body.innerHTML = '<p>hello</p>';
    expect(detect(doc, 'application/json')).toBeNull();
  });

  it('leaves an HTML page alone', () => {
    expect(detect(jsonDocument('{}'), 'text/html')).toBeNull();
  });

  it('takes an XML document as it is', () => {
    const doc = xmlDocument('<urlset><url><loc>https://a.test/</loc></url></urlset>');
    const found = detect(doc, 'application/xml');
    expect(found?.kind).toBe('xml');
    expect(found && found.kind === 'xml' ? found.nodes.map((node) => node.nodeName) : []).toEqual(['urlset']);
  });

  it("takes the source out of Chromium's XML tree viewer", () => {
    const doc = document.implementation.createHTMLDocument('');
    const source = doc.createElement('div');
    source.id = XML_SOURCE_ID;
    const xml = xmlDocument('<feed><title>t</title></feed>');
    source.append(doc.importNode(xml.documentElement, true));
    doc.body.append(source);
    const found = detect(doc, 'text/xml');
    expect(found && found.kind === 'xml' ? found.nodes.map((node) => node.nodeName) : []).toEqual(['feed']);
  });

  it('leaves an XML document with a style sheet to its style sheet', () => {
    const doc = xmlDocument('<rss/>');
    doc.insertBefore(doc.createProcessingInstruction('xml-stylesheet', 'href="s.xsl"'), doc.documentElement);
    expect(detect(doc, 'application/rss+xml')).toBeNull();
  });

  it("leaves an XML document with a parser error to Chromium's own message", () => {
    const doc = xmlDocument('<a><b></a>');
    expect(detect(doc, 'application/xml')).toBeNull();
  });
});
