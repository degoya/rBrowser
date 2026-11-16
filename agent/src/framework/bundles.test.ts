import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

// The main-world bundles `npm run build` wrote (vitest runs in `agent/`): the Framework
// DevTools backends open no port and no socket - their bridges run over DOM events only.
const BUNDLES = ['rb-framework-probe.js', 'rb-react-devtools.js', 'rb-vue-devtools.js'];

describe('the Framework DevTools bundles', () => {
  it('open no port and no socket', () => {
    for (const name of BUNDLES) {
      const text = readFileSync(resolve(process.cwd(), 'dist', name), 'utf8');
      expect(text.length).toBeGreaterThan(0);
      expect(text.includes('new WebSocket('), `${name} opens a WebSocket`).toBe(false);
      expect(text.includes('EventSource('), `${name} opens an EventSource`).toBe(false);
      expect(text.includes('ws://'), `${name} names a ws:// address`).toBe(false);
    }
  });
});
