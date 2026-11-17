import { describe, expect, it } from 'vitest';
import { evaluateJsonPath, parseJsonPath } from './jsonpath';
import { JsonNode } from './nodes';

const DOC = {
  items: [
    { id: 1, tags: ['a', 'b'] },
    { id: 2, tags: [] },
    { id: 3, deep: { id: 'x' } },
  ],
  'odd key': { "it's": 7 },
  count: 3,
};

function run(query: string): unknown[] {
  const answer = evaluateJsonPath(new JsonNode(DOC), query);
  if (!answer.ok) throw new Error(`query failed at ${answer.offset}`);
  return answer.nodes.map((node) => node.path());
}

describe('JSONPath', () => {
  it('selects a child by name, index and wildcard', () => {
    expect(run('$')).toEqual(['$']);
    expect(run('$.count')).toEqual(['$.count']);
    expect(run('$.items[*].id')).toEqual(['$.items[0].id', '$.items[1].id', '$.items[2].id']);
    expect(run('$.items[-1].id')).toEqual(['$.items[2].id']);
    expect(run('$.items.*.id')).toEqual(['$.items[0].id', '$.items[1].id', '$.items[2].id']);
  });

  it('takes quoted names, unions and slices', () => {
    expect(run("$['odd key'][\"it's\"]")).toEqual(["$['odd key']['it\\'s']"]);
    expect(run("$['count','items'][0]")).toEqual(['$.items[0]']);
    expect(run('$.items[0,2].id')).toEqual(['$.items[0].id', '$.items[2].id']);
    expect(run('$.items[1:].id')).toEqual(['$.items[1].id', '$.items[2].id']);
    expect(run('$.items[::2].id')).toEqual(['$.items[0].id', '$.items[2].id']);
    expect(run('$.items[:-1].id')).toEqual(['$.items[0].id', '$.items[1].id']);
  });

  it('descends with ..', () => {
    expect(run('$..id')).toEqual(['$.items[0].id', '$.items[1].id', '$.items[2].id', '$.items[2].deep.id']);
    expect(run('$..tags[0]')).toEqual(['$.items[0].tags[0]']);
    expect(run('$.items[0]..*')).toEqual(['$.items[0].id', '$.items[0].tags', '$.items[0].tags[0]', '$.items[0].tags[1]']);
  });

  it('answers nothing for a name that is not there', () => {
    expect(run('$.missing.id')).toEqual([]);
    expect(run('$.count[0]')).toEqual([]);
  });

  it('refuses filters and malformed queries with the offset', () => {
    expect(parseJsonPath('$.items[?(@.id>1)]')).toEqual({ ok: false, offset: 8 });
    expect(parseJsonPath('items')).toEqual({ ok: false, offset: 0 });
    expect(parseJsonPath('$.items[')).toEqual({ ok: false, offset: 8 });
    expect(parseJsonPath("$['open")).toEqual({ ok: false, offset: 7 });
    expect(parseJsonPath('$.')).toEqual({ ok: false, offset: 2 });
  });

  it('stops at the result limit and says so', () => {
    const big = new JsonNode(Array.from({ length: 50 }, (_, id) => ({ id })));
    const answer = evaluateJsonPath(big, '$[*].id', 10);
    expect(answer.ok && answer.nodes.length).toBe(10);
    expect(answer.ok && answer.truncated).toBe(true);
  });
});
