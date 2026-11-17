// RB-070-06 in the command palette (RB-060-11): "Open bookmark: <label>" for every bookmark that
// leads somewhere in the current environment; choosing one runs `bookmarks.open`.

import type { PaletteEntry, PaletteProvider } from '../palette/types';
import { OPEN_COMMAND, bookmarksOf, openMessage } from './bookmarks';

export const bookmarkProvider: PaletteProvider = {
  id: 'bookmarks',
  entries({ state, t }): PaletteEntry[] {
    return bookmarksOf(state)
      .filter((bookmark) => bookmark.url !== undefined)
      .map((bookmark) => ({
        key: `bookmark:${bookmark.id}`,
        scope: 'other',
        title: t('palette.entry.open_bookmark', { label: bookmark.label }),
        group: t('palette.kind.bookmark'),
        ...(bookmark.url === undefined ? {} : { description: bookmark.url }),
        keywords: bookmark.group === undefined ? [bookmark.id] : [bookmark.id, bookmark.group],
        ...(bookmark.shortcut === undefined ? {} : { shortcut: bookmark.shortcut }),
        commandId: OPEN_COMMAND,
        action: { kind: 'send', message: openMessage(bookmark.id) },
      }));
  },
};
