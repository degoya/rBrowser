import { describe, expect, it } from 'vitest';
import { blocksOf, hintOf, references, saveOf, type VariablesAnswer } from './variables';
import { emptyRequest } from './model';

const ANSWER: VariablesAnswer = {
  file: { $shared: { user: 'ada' }, staging: { path: '/v1', retries: 3 } },
  private: { staging: ['token'], $shared: ['password'] },
  baseUrls: { staging: 'https://staging.example.com', production: 'https://api.example.com' },
};

describe('the Variables tab (RB-096-11)', () => {
  it('shows $shared first, then each environment, the private rows by name only', () => {
    const blocks = blocksOf(ANSWER);
    expect(blocks.map((block) => block.name)).toEqual(['$shared', 'staging']);
    expect(blocks[1]?.rows).toEqual([
      { name: 'path', value: '/v1', secret: false, stored: false },
      { name: 'retries', value: '3', secret: false, stored: false },
      { name: 'token', value: '', secret: true, stored: true },
    ]);
  });

  it('writes the public rows to the file and only the new private values to the patch', () => {
    const blocks = blocksOf(ANSWER);
    const staging = blocks[1]!;
    staging.rows.push({ name: 'apiKey', value: 'k-1', secret: true, stored: false });
    staging.rows = staging.rows.filter((row) => row.name !== 'retries');
    blocks[0]!.rows = blocks[0]!.rows.filter((row) => row.name !== 'password');
    const op = saveOf(blocks, ANSWER);
    expect(op.file).toEqual({ $shared: { user: 'ada' }, staging: { path: '/v1' } });
    expect(op.base).toEqual(ANSWER.file);
    expect(op.private).toEqual({ staging: { apiKey: 'k-1' }, $shared: { password: null } });
    expect(op.renamed).toEqual({});
  });

  it('takes a renamed environment and its private values along, and drops a deleted one', () => {
    const blocks = blocksOf(ANSWER);
    blocks[1]!.name = 'stage';
    let op = saveOf(blocks, ANSWER);
    expect(op.renamed).toEqual({ staging: 'stage' });
    expect(op.file).toEqual({ $shared: { user: 'ada' }, stage: { path: '/v1', retries: 3 } });
    expect(op.private).toEqual({});
    expect(op.deleted).toEqual([]);
    op = saveOf(blocks.slice(0, 1), ANSWER);
    expect(op.private).toEqual({});
    expect(op.deleted).toEqual(['staging']);
    expect(Object.keys(op.file as object)).toEqual(['$shared']);
  });

  it('deletes an environment and renames another onto its name in one save (code review w33)', () => {
    const answer: VariablesAnswer = { file: { dev: { a: '1' }, stage: { b: '2' } }, private: { dev: ['x'], stage: ['y'] }, baseUrls: {} };
    const blocks = blocksOf(answer).filter((block) => block.name !== 'dev');
    const stage = blocks.find((block) => block.name === 'stage')!;
    stage.name = 'dev';
    stage.rows.push({ name: 'z', value: 'new', secret: true, stored: false });
    const op = saveOf(blocks, answer);
    expect(op.renamed).toEqual({ stage: 'dev' });
    expect(op.deleted).toEqual(['dev']);
    // The renamed block's own change stays, and no drop by name touches the values it brings.
    expect(op.private).toEqual({ dev: { z: 'new' } });
  });

  it('drops the private values of a deleted environment and keeps those of a new one of its name', () => {
    const blocks = blocksOf(ANSWER).filter((block) => block.name !== 'staging');
    blocks.push({ name: 'staging', original: null, rows: [{ name: 'apiKey', value: 'k-2', secret: true, stored: false }] });
    const op = saveOf(blocks, ANSWER);
    expect(op.deleted).toEqual(['staging']);
    expect(op.private).toEqual({ staging: { apiKey: 'k-2' } });
  });

  it('keeps the JSON type of a number or boolean, edited or not', () => {
    const answer: VariablesAnswer = { file: { dev: { retries: 3, debug: true, port: 8080, name: '7' } }, private: {}, baseUrls: {} };
    const blocks = blocksOf(answer);
    const rows = blocks[1]!.rows;
    rows[2]!.value = '9090';
    const op = saveOf(blocks, answer);
    expect(op.file.dev).toEqual({ retries: 3, debug: true, port: 9090, name: '7' });
    rows[0]!.value = 'three';
    rows[1]!.value = 'false';
    expect(saveOf(blocks, answer).file.dev).toEqual({ retries: 'three', debug: false, port: 9090, name: '7' });
  });

  it('finds the variables a request names, the secrets apart', () => {
    const request = { ...emptyRequest(), url: '{{baseUrl}}/users/{{ id }}?k={{$secret api-key}}', headers: [{ name: 'X', value: '{{token}} {{$guid}}', enabled: true }] };
    request.auth = { type: 'basic', username: '{{user}}', password: '{{$secret password}}' };
    expect(references(request)).toEqual({ plain: ['baseUrl', 'id', 'token', 'user'], secrets: ['api-key', 'password'] });
  });

  it("tells a variable's value in the environment, a secret's by name, an unknown one", () => {
    const blocks = blocksOf(ANSWER);
    expect(hintOf('path', 'staging', blocks, ANSWER.baseUrls)).toEqual({ state: 'plain', value: '/v1' });
    expect(hintOf('user', 'staging', blocks, ANSWER.baseUrls)).toEqual({ state: 'plain', value: 'ada' });
    expect(hintOf('token', 'staging', blocks, ANSWER.baseUrls)).toEqual({ state: 'secret' });
    expect(hintOf('baseUrl', 'production', blocks, ANSWER.baseUrls)).toEqual({ state: 'plain', value: 'https://api.example.com' });
    expect(hintOf('path', 'production', blocks, ANSWER.baseUrls)).toEqual({ state: 'unknown' });
  });
});
