<script setup lang="ts">
// The toolbar's bookmarks (RB-070-06): a pulldown of the project's bookmarks, grouped by `group`
// under headings, as the core's native menu below the button (RB-090-45, `PulldownButton.vue`) -
// in rBrowser's theme, never Chromium's `<select>` popup - and "Bookmark this page". Choosing one
// runs `bookmarks.open`; the search lives in the quick pick (`Leader P B`, `BookmarkBar.vue`) and
// the command palette. A bookmark that follows the environment shows its path (RB-080-48); one
// without a URL is shown and cannot be chosen.
//
// A long list is cut at `BOOKMARK_ROWS` rows, headings included: the last row then counts the
// bookmarks left out and opens the quick pick, which searches them all. A menu of hundreds of rows
// is no way to find one, and the core takes at most 200 rows (`native_menu::MAX_ROWS`). A row's id
// is the bookmark's place in the menu, `open:<n>`, never its id: the core takes row ids of at most
// `MAX_TEXT` characters, and one long bookmark id must not break the whole menu (RB-090-54).
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import type { BookmarkSummary, NativeMenuRow } from '../ipc/protocol';
import { ICONS } from '../icons';
import { useShell } from '../state/shell';
import { ADD_COMMAND, OPEN_COMMAND, bookmarksOf, groupBookmarks, openMessage } from '../bookmarks/bookmarks';
import { capRows } from './nativeMenu';
import PulldownButton from './PulldownButton.vue';

/** The most rows of the menu; the last one opens the search for the rest. */
const BOOKMARK_ROWS = 100;
/** The row ids: a bookmark's place in the menu, and the search for those left out. */
const OPEN = 'open:';
const SEARCH = 'search';

/** The bookmark id of each row last built: what its `open:<n>` opens. */
let shown = new Map<string, string>();

const shell = useShell();
const { t } = useI18n();

const bookmarks = computed(() => bookmarksOf(shell.state.value));
const groups = computed(() => groupBookmarks(bookmarks.value));

/** The row's text: the label, with the path of a bookmark that follows the environment. */
function textOf(bookmark: BookmarkSummary): string {
  return bookmark.path === undefined
    ? bookmark.label
    : t('bookmarks.item.relative', { label: bookmark.label, path: bookmark.path });
}

/** The groups under their names, a line between two; the search last where the list is cut. */
function rows(): NativeMenuRow[] {
  const out: NativeMenuRow[] = [];
  const ids = new Map<string, string>();
  for (const group of groups.value) {
    if (out.length > 0) {
      out.push({ kind: 'separator' });
    }
    if (group.name !== null) {
      out.push({ kind: 'heading', label: group.name });
    }
    for (const bookmark of group.items) {
      const id = `${OPEN}${ids.size}`;
      ids.set(id, bookmark.id);
      out.push({ kind: 'item', id, label: textOf(bookmark), disabled: bookmark.url === undefined });
    }
  }
  shown = ids;
  return capRows(out, BOOKMARK_ROWS, (count) => ({ kind: 'item', id: SEARCH, label: t('bookmarks.menu.more', { count }) }));
}

function choose(id: string): void {
  if (id === SEARCH) {
    void shell.send({ type: 'runCommand', id: OPEN_COMMAND });
  } else {
    const bookmark = shown.get(id);
    if (bookmark !== undefined) {
      void shell.send(openMessage(bookmark));
    }
  }
}
</script>

<template>
  <div class="flex shrink-0 items-center gap-0.5" data-testid="bookmarks">
    <PulldownButton
      kind="bookmarks"
      :label="bookmarks.length === 0 ? t('bookmarks.menu.empty') : t('bookmarks.menu.label')"
      :rows="rows"
      :disabled="bookmarks.length === 0"
      data-testid="bookmarks-menu"
      @choose="choose"
    />
    <UButton
      :icon="ICONS.bookmarkAdd"
      color="neutral"
      variant="ghost"
      size="sm"
      square
      :aria-label="t('bookmarks.menu.add')"
      :title="t('bookmarks.menu.add')"
      data-testid="bookmarks-add"
      @click="shell.send({ type: 'runCommand', id: ADD_COMMAND })"
    />
  </div>
</template>
