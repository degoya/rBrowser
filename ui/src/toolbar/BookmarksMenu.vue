<script setup lang="ts">
// The toolbar's bookmarks (RB-070-06): a native select of the project's bookmarks, grouped by
// `group` (CEF opens its list as a native popup that may cover the views; an HTML list would be
// clipped by the shell view), and "Bookmark this page". Choosing one runs `bookmarks.open`; the
// search lives in the quick pick (`Leader P B`, `BookmarkBar.vue`) and the command palette. A
// bookmark that follows the environment shows its path; the tooltip is where it leads now
// (RB-080-48). An arrow key or a typed letter on the closed select browses, it does not open a
// bookmark: such a key opens the list instead (`menuKeys.ts`, RB-090-23).
import { computed, ref, useId } from 'vue';
import type { BookmarkSummary } from '../ipc/protocol';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import { ICONS } from '../icons';
import { useShell } from '../state/shell';
import { createMenuKeys } from './menuKeys';
import { ADD_COMMAND, bookmarksOf, groupBookmarks, openMessage } from '../bookmarks/bookmarks';

const shell = useShell();
const { t } = useI18n();
const id = useId();

const bookmarks = computed(() => bookmarksOf(shell.state.value));
const groups = computed(() => groupBookmarks(bookmarks.value));
const select = ref<HTMLSelectElement | null>(null);

/** The option's text: the label, with the path of a bookmark that follows the environment. */
function textOf(bookmark: BookmarkSummary): string {
  return bookmark.path === undefined
    ? bookmark.label
    : t('bookmarks.item.relative', { label: bookmark.label, path: bookmark.path });
}

const keys = createMenuKeys();

function onChange(event: Event): void {
  const element = event.target as HTMLSelectElement;
  const chosen = element.value;
  // The select names the list, not a current bookmark: it goes back to its label.
  element.value = '';
  if (chosen !== '' && !keys.browsing()) {
    void shell.send(openMessage(chosen));
  }
}
</script>

<template>
  <div class="flex shrink-0 items-center gap-0.5" data-testid="bookmarks">
    <label :for="id" class="sr-only">{{ t('bookmarks.menu.label') }}</label>
    <select
      :id="id"
      ref="select"
      :title="t('bookmarks.menu.label')"
      :disabled="bookmarks.length === 0"
      value=""
      class="h-8 max-w-40 rounded-md bg-default px-2 text-sm text-default ring ring-inset ring-accented focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary disabled:cursor-not-allowed disabled:opacity-75"
      data-testid="bookmarks-select"
      @pointerdown="keys.onPointerDown"
      @keydown="keys.onKeyDown"
      @keyup="keys.release"
      @blur="keys.release"
      @change="onChange"
    >
      <option value="" disabled>
        {{ bookmarks.length === 0 ? t('bookmarks.menu.empty') : t('bookmarks.menu.label') }}
      </option>
      <template v-for="group in groups" :key="group.name ?? ''">
        <optgroup v-if="group.name !== null" :label="group.name">
          <option
            v-for="b in group.items"
            :key="b.id"
            :value="b.id"
            :disabled="b.url === undefined"
            :title="b.url ?? t('bookmarks.no_url')"
          >
            {{ textOf(b) }}
          </option>
        </optgroup>
        <template v-else>
          <option
            v-for="b in group.items"
            :key="b.id"
            :value="b.id"
            :disabled="b.url === undefined"
            :title="b.url ?? t('bookmarks.no_url')"
          >
            {{ textOf(b) }}
          </option>
        </template>
      </template>
    </select>
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
