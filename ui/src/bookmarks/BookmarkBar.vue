<script setup lang="ts">
// The bookmarks' quick pick and the "Bookmark this page" dialog of RB-070-06. `bookmarks.open`
// and `bookmarks.add` without arguments (their leader chords, the toolbar button) reach the shell
// as `uiCommand`; this bar covers the toolbar strip (the shell view, never a page). The quick pick
// searches label, group and URL; Enter opens the selected match, the arrow keys move the
// selection. The dialog proposes the active page's URL and sends what was typed with its switch
// "Follows the environment" (on as `bookmarks.followEnvironment` says, RB-080-48): the core
// stores the path when the target has one under the environments, else the URL - the shell
// computes neither. Escape closes. Mounted in `App.vue`.
// Closed by Escape, Cancel or done, the focus goes back to its opener (`returnFocus.ts`, RB-090-23).
import { computed, nextTick, onScopeDispose, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { returnFocus } from '../focus/returnFocus';
import { useShell } from '../state/shell';
import { windowEvents, type Subscribe } from '../whichkey/whichkey';
import {
  ADD_COMMAND,
  OPEN_COMMAND,
  addMessage,
  bookmarksOf,
  filterBookmarks,
  followsEnvironment,
  openMessage,
  targetOf,
  type BookmarkScope,
} from './bookmarks';

/** How many matches the strip shows beside the field. */
const SHOWN = 8;

const props = withDefaults(defineProps<{ subscribe?: Subscribe }>(), { subscribe: undefined });
const shell = useShell();
const { t } = useI18n();

const mode = ref<'open' | 'add' | null>(null);
const query = ref('');
const selected = ref(0);
const label = ref('');
const target = ref('');
const group = ref('');
const scope = ref<BookmarkScope>('project');
const follow = ref(true);
const invalid = ref(false);
const bar = ref<HTMLElement | null>(null);
const focus = returnFocus();

const matches = computed(() => filterBookmarks(bookmarksOf(shell.state.value), query.value).slice(0, SHOWN));
const activeTitle = computed(() => {
  const state = shell.state.value;
  return state?.views.find((v) => v.id === state.activeView)?.title ?? '';
});

const stop = (props.subscribe ?? windowEvents)((message) => {
  if (message.type !== 'uiCommand') {
    return;
  }
  if (message.id === OPEN_COMMAND) {
    query.value = '';
    selected.value = 0;
    mode.value = 'open';
  } else if (message.id === ADD_COMMAND) {
    target.value = shell.state.value?.url ?? '';
    follow.value = followsEnvironment(shell.settings.value);
    label.value = activeTitle.value;
    group.value = '';
    scope.value = 'project';
    invalid.value = false;
    mode.value = 'add';
  } else {
    return;
  }
  focus.remember();
  void nextTick(() => bar.value?.querySelector<HTMLElement>('input')?.focus());
});
onScopeDispose(stop);

function close(): void {
  mode.value = null;
  focus.restore();
}

function move(by: 1 | -1): void {
  const count = matches.value.length;
  if (count > 0) {
    selected.value = (selected.value + by + count) % count;
  }
}

async function open(id: string | undefined): Promise<void> {
  if (id !== undefined && (await shell.send(openMessage(id)))) {
    close();
  }
}

async function save(): Promise<void> {
  const parsed = targetOf(target.value);
  if (parsed === null) {
    invalid.value = true;
    return;
  }
  const draft = { label: label.value, target: parsed, follow: follow.value, group: group.value, scope: scope.value };
  if (await shell.send(addMessage(draft))) {
    close();
  }
}
</script>

<template>
  <div
    v-if="mode !== null"
    ref="bar"
    role="dialog"
    :aria-label="mode === 'open' ? t('commands.bookmarks.open') : t('commands.bookmarks.add')"
    class="fixed inset-x-0 top-0 z-50 flex h-11 items-center gap-2 overflow-x-auto border-b border-default bg-elevated px-3 text-sm text-default"
    data-testid="bookmark-bar"
    @keydown.escape.prevent="close"
  >
    <template v-if="mode === 'open'">
      <label for="rb-bookmark-search" class="shrink-0 text-muted">{{ t('bookmarks.open.label') }}</label>
      <input
        id="rb-bookmark-search"
        v-model="query"
        type="search"
        autocomplete="off"
        spellcheck="false"
        class="w-48 shrink-0 rounded-sm border border-default bg-default px-2 py-1 focus-visible:outline-2 focus-visible:outline-primary"
        :placeholder="t('bookmarks.open.placeholder')"
        aria-controls="rb-bookmark-matches"
        data-testid="bookmark-search"
        @input="selected = 0"
        @keydown.down.prevent="move(1)"
        @keydown.up.prevent="move(-1)"
        @keydown.enter.prevent="open(matches[selected]?.id)"
      />
      <div id="rb-bookmark-matches" role="group" :aria-label="t('bookmarks.open.matches')" class="flex min-w-0 items-center gap-1">
        <span v-if="matches.length === 0" class="shrink-0 text-muted" data-testid="bookmark-none">
          {{ t('bookmarks.open.none') }}
        </span>
        <button
          v-for="(bookmark, index) in matches"
          :key="bookmark.id"
          type="button"
          class="flex shrink-0 items-center gap-1 rounded-sm border px-2 py-0.5 focus-visible:outline-2 focus-visible:outline-primary"
          :class="index === selected ? 'border-primary bg-default' : 'border-default'"
          :aria-current="index === selected ? 'true' : undefined"
          :title="bookmark.url ?? t('bookmarks.no_url')"
          :disabled="bookmark.url === undefined"
          :data-testid="`bookmark-match-${bookmark.id}`"
          @click="open(bookmark.id)"
        >
          <span v-if="bookmark.group" class="text-muted">{{ t('bookmarks.open.group', { group: bookmark.group }) }}</span>
          {{ bookmark.label }}
          <span v-if="bookmark.path" class="font-mono text-xs text-muted">{{ bookmark.path }}</span>
        </button>
      </div>
    </template>
    <form v-else class="flex min-w-0 items-center gap-2" data-testid="bookmark-add" @submit.prevent="save">
      <label for="rb-bookmark-label" class="shrink-0 text-muted">{{ t('bookmarks.add.label') }}</label>
      <input
        id="rb-bookmark-label"
        v-model="label"
        type="text"
        autocomplete="off"
        class="w-36 shrink-0 rounded-sm border border-default bg-default px-2 py-1 focus-visible:outline-2 focus-visible:outline-primary"
        data-testid="bookmark-add-label"
      />
      <label for="rb-bookmark-target" class="shrink-0 text-muted">{{ t('bookmarks.add.target') }}</label>
      <input
        id="rb-bookmark-target"
        v-model="target"
        type="text"
        autocomplete="off"
        spellcheck="false"
        class="w-56 shrink-0 rounded-sm border bg-default px-2 py-1 font-mono focus-visible:outline-2 focus-visible:outline-primary"
        :class="invalid ? 'border-error' : 'border-default'"
        :aria-invalid="invalid ? 'true' : undefined"
        aria-describedby="rb-bookmark-target-hint"
        data-testid="bookmark-add-target"
        @input="invalid = false"
      />
      <span id="rb-bookmark-target-hint" class="shrink-0 text-xs" :class="invalid ? 'text-error' : 'text-muted'">
        {{ invalid ? t('bookmarks.add.invalid') : t('bookmarks.add.hint') }}
      </span>
      <label class="flex shrink-0 items-center gap-1 text-muted" :title="t('bookmarks.add.follow_hint')">
        <input
          v-model="follow"
          type="checkbox"
          class="focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary"
          data-testid="bookmark-add-follow"
        />
        {{ t('bookmarks.add.follow') }}
      </label>
      <label for="rb-bookmark-group" class="shrink-0 text-muted">{{ t('bookmarks.add.group') }}</label>
      <input
        id="rb-bookmark-group"
        v-model="group"
        type="text"
        autocomplete="off"
        class="w-28 shrink-0 rounded-sm border border-default bg-default px-2 py-1 focus-visible:outline-2 focus-visible:outline-primary"
        data-testid="bookmark-add-group"
      />
      <label for="rb-bookmark-scope" class="shrink-0 text-muted">{{ t('bookmarks.add.scope') }}</label>
      <select
        id="rb-bookmark-scope"
        v-model="scope"
        class="shrink-0 rounded-sm border border-default bg-default px-2 py-0.5 focus-visible:outline-2 focus-visible:outline-primary"
        data-testid="bookmark-add-scope"
      >
        <option value="project">{{ t('bookmarks.scope.project') }}</option>
        <option value="local">{{ t('bookmarks.scope.local') }}</option>
      </select>
      <button
        type="submit"
        class="shrink-0 rounded-sm bg-primary px-2 py-0.5 text-inverted focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary"
        data-testid="bookmark-add-save"
      >
        {{ t('bookmarks.add.save') }}
      </button>
    </form>
    <button
      type="button"
      class="ml-auto shrink-0 rounded-sm px-2 py-0.5 text-muted focus-visible:outline-2 focus-visible:outline-primary"
      data-testid="bookmark-cancel"
      @click="close"
    >
      {{ t('bookmarks.cancel') }}
    </button>
  </div>
</template>
