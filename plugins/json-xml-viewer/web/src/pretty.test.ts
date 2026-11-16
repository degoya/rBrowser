import { describe, expect, it } from 'vitest';
import { prettyXml, serializeXml } from './pretty';

function nodes(text: string): Node[] {
  return [...new DOMParser().parseFromString(text, 'application/xml').childNodes];
}

describe('the XML texts', () => {
  it('indents elements and keeps short text inline', () => {
    const text = '<feed a="1&amp;2"><title>A &lt; B</title><entry><id>1</id><!--c--><empty/></entry></feed>';
    expect(prettyXml(nodes(text))).toBe(
      [
        '<feed a="1&amp;2">',
        '  <title>A &lt; B</title>',
        '  <entry>',
        '    <id>1</id>',
        '    <!--c-->',
        '    <empty/>',
        '  </entry>',
        '</feed>',
      ].join('\n'),
    );
  });

  it('serialises the source nodes as they are', () => {
    expect(serializeXml(nodes('<a><b>x</b></a>'))).toBe('<a><b>x</b></a>');
  });
});
