import { describe, expect, it } from 'vitest';
import { cookiesOf } from './cookies';

describe('the cookies of an answer', () => {
  it('reads every Set-Cookie with its attributes', () => {
    const cookies = cookiesOf([
      ['content-type', 'text/plain'],
      ['set-cookie', 'sid=abc=1; Path=/; HttpOnly; SameSite=Lax'],
      ['Set-Cookie', 'flag'],
    ]);
    expect(cookies).toEqual([
      {
        name: 'sid',
        value: 'abc=1',
        attributes: [
          { name: 'Path', value: '/' },
          { name: 'HttpOnly', value: '' },
          { name: 'SameSite', value: 'Lax' },
        ],
      },
      { name: 'flag', value: '', attributes: [] },
    ]);
  });
});
