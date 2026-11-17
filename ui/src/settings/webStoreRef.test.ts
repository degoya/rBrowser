import { describe, expect, it } from 'vitest';
import { isExtensionId, parseStoreRef } from './webStoreRef';

const ID = 'nhdogjmejiglipccpnnnanhbledajbpd';

describe('a Web Store reference', () => {
  it('is an id or a store page URL', () => {
    expect(isExtensionId(ID)).toBe(true);
    expect(parseStoreRef(` ${ID}\n`)).toBe(ID);
    expect(parseStoreRef(`https://chromewebstore.google.com/detail/vuejs-devtools/${ID}?hl=de`)).toBe(ID);
    expect(parseStoreRef(`https://chromewebstore.google.com/detail/${ID}/`)).toBe(ID);
    expect(parseStoreRef(`https://chrome.google.com/webstore/detail/${ID}`)).toBe(ID);
  });

  it('is nothing else', () => {
    expect(parseStoreRef('')).toBeNull();
    expect(parseStoreRef('abc')).toBeNull();
    expect(parseStoreRef(ID.toUpperCase())).toBeNull();
    expect(parseStoreRef(ID.replace('n', 'q'))).toBeNull();
    expect(parseStoreRef(`https://evil.test/detail/x/${ID}`)).toBeNull();
    expect(parseStoreRef(`http://chromewebstore.google.com/detail/x/${ID}`)).toBeNull();
  });
});
