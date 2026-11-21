import { describe, expect, it } from 'vitest';
import { MAX_IMPORT_BYTES, filesOf, firstOf, importOp, nameOf, notesByKind, withinLimit } from './importing';

describe('the API client import', () => {
  it('names a folder file by its relative path', () => {
    const plain = new File(['{}'], 'a.json');
    expect(nameOf(plain)).toBe('a.json');
    const inFolder = new File(['meta {}'], 'get.bru');
    Object.defineProperty(inFolder, 'webkitRelativePath', { value: 'library/users/get.bru' });
    expect(nameOf(inFolder)).toBe('library/users/get.bru');
  });

  it('reads the files and puts the pasted text last', async () => {
    const files = await filesOf([new File(['{"a":1}'], 'a.json')], 'curl https://a.example');
    expect(files).toEqual([
      { name: 'a.json', text: '{"a":1}' },
      { name: '', text: 'curl https://a.example' },
    ]);
    expect(await filesOf([], '  ')).toEqual([]);
  });

  it('holds the size before anything is sent', () => {
    expect(withinLimit([new File(['x'], 'a.json')], 'y')).toBe(true);
    expect(withinLimit([], 'x'.repeat(MAX_IMPORT_BYTES + 1))).toBe(false);
  });

  it('builds the op with auto and an empty folder as none', () => {
    expect(importOp([], { format: 'auto', folder: '  ', environments: true, preview: true })).toEqual({
      op: 'import',
      files: [],
      format: null,
      folder: null,
      environments: true,
      preview: true,
    });
    expect(importOp([], { format: 'har', folder: 'shop', environments: false, preview: false })).toMatchObject({
      format: 'har',
      folder: 'shop',
      environments: false,
    });
  });

  it('splits the notes and cuts long lists', () => {
    const notes = notesByKind({
      imported: 1,
      environments: 0,
      secrets: [],
      notes: [
        { kind: 'reported', code: 'script', at: 'A', detail: 'pre-request' },
        { kind: 'dropped', code: 'item', at: 'B', detail: 'websocket_request' },
      ],
    });
    expect(notes.reported.map((n) => n.at)).toEqual(['A']);
    expect(notes.dropped.map((n) => n.at)).toEqual(['B']);
    expect(firstOf([1, 2, 3], 2)).toEqual({ shown: [1, 2], more: 1 });
    expect(firstOf([1], 2)).toEqual({ shown: [1], more: 0 });
  });
});
