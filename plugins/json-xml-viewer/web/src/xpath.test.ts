import { describe, expect, it } from 'vitest';
import { xmlRoots } from './fixtures';
import { evaluateXPath } from './xpath';

const SITEMAP =
  '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:x="http://x.test/">' +
  '<url><loc>https://a.test/</loc><priority>1.0</priority></url>' +
  '<url lang="de"><loc>https://a.test/de</loc><x:alt href="h"/></url>' +
  '<url lang="fr"><loc>https://a.test/fr</loc><!-- old --></url>' +
  '</urlset>';

function run(query: string, text = SITEMAP): string[] {
  const answer = evaluateXPath(xmlRoots(text), query);
  if (!answer.ok) throw new Error(`query failed at ${answer.offset}`);
  return answer.nodes.map((node) => node.path());
}

describe('XPath', () => {
  it('matches unprefixed names by local name, so a default namespace needs no prefix', () => {
    expect(run('//url/loc')).toEqual(['/urlset/url[1]/loc', '/urlset/url[2]/loc', '/urlset/url[3]/loc']);
    expect(run('/urlset/url')).toEqual(['/urlset/url[1]', '/urlset/url[2]', '/urlset/url[3]']);
    expect(run('//x:alt')).toEqual(['/urlset/url[2]/x:alt']);
  });

  it('takes positions, last() and attribute predicates', () => {
    expect(run('//url[2]/loc')).toEqual(['/urlset/url[2]/loc']);
    expect(run('//url[last()]')).toEqual(['/urlset/url[3]']);
    expect(run('//url[last()-1]')).toEqual(['/urlset/url[2]']);
    expect(run('//url[@lang]')).toEqual(['/urlset/url[2]', '/urlset/url[3]']);
    expect(run("//url[@lang='fr']/loc")).toEqual(['/urlset/url[3]/loc']);
    expect(run("//url[@lang!='fr' and @lang]")).toEqual(['/urlset/url[2]']);
    expect(run("//url[loc='https://a.test/']")).toEqual(['/urlset/url[1]']);
    expect(run("//url[contains(loc, '/de')]")).toEqual(['/urlset/url[2]']);
    expect(run("//url[not(@lang)]")).toEqual(['/urlset/url[1]']);
    expect(run("//loc[starts-with(., 'https://a.test/f')]")).toEqual(['/urlset/url[3]/loc']);
  });

  it('selects attributes, text, comments, parents and unions', () => {
    expect(run('//url/@lang')).toEqual(['/urlset/url[2]/@lang', '/urlset/url[3]/@lang']);
    expect(run('//url[1]/loc/text()')).toEqual(['/urlset/url[1]/loc/text()']);
    expect(run('//comment()')).toEqual(['/urlset/url[3]/comment()']);
    expect(run('//x:alt/..')).toEqual(['/urlset/url[2]']);
    expect(run('//priority | //x:alt')).toEqual(['/urlset/url[1]/priority', '/urlset/url[2]/x:alt']);
    expect(run('/urlset/*[1]/*')).toEqual(['/urlset/url[1]/loc', '/urlset/url[1]/priority']);
  });

  it('answers nothing for a name that is not there', () => {
    expect(run('//missing')).toEqual([]);
  });

  it('refuses a malformed query with the offset', () => {
    expect(evaluateXPath(xmlRoots(SITEMAP), '//url[')).toEqual({ ok: false, offset: 6 });
    expect(evaluateXPath(xmlRoots(SITEMAP), '//url[@lang=]')).toEqual({ ok: false, offset: 12 });
    expect(evaluateXPath(xmlRoots(SITEMAP), '//')).toEqual({ ok: false, offset: 2 });
  });

  it('stops at the result limit and says so', () => {
    const many = `<r>${'<i/>'.repeat(30)}</r>`;
    const answer = evaluateXPath(xmlRoots(many), '//i', 5);
    expect(answer.ok && answer.nodes.length).toBe(5);
    expect(answer.ok && answer.truncated).toBe(true);
  });
});
