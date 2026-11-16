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
// Nuxt UI since RB-080-88: `UInput`s, `UButton`s, and where it is saved is the `USwitch` "Share
// with the team" (on: the project file) - a select would open a popover the strip cuts off.
import { computed, nextTick, onScopeDispose, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import UInput from '@nuxt/ui/components/Input.vue';
import USwitch from '@nuxt/ui/components/Switch.vue';
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
const share = computed({
  get: () => scope.value === 'project',
  set: (on: boolean) => (scope.value = on ? 'project' : 'local'),
});
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
      <UInput
        id="rb-bookmark-search"
        v-model="query"
        type="search"
        size="sm"
        class="w-48 shrink-0"
        autocomplete="off"
        spellcheck="false"
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
        <UButton
          v-for="(bookmark, index) in matches"
          :key="bookmark.id"
          size="sm"
          variant="outline"
          :color="index === selected ? 'primary' : 'neutral'"
          class="shrink-0"
          :aria-current="index === selected ? 'true' : undefined"
          :title="bookmark.url ?? t('bookmarks.no_url')"
          :disabled="bookmark.url === undefined"
          :data-testid="`bookmark-match-${bookmark.id}`"
          @click="open(bookmark.id)"
        >
          <span v-if="bookmark.group" class="text-muted">{{ t('bookmarks.open.group', { group: bookmark.group }) }}</span>
          {{ bookmark.label }}
          <span v-if="bookmark.path" class="font-mono text-xs text-muted">{{ bookmark.path }}</span>
        </UButton>
      </div>
    </template>
    <form v-else class="flex min-w-0 items-center gap-2" data-testid="bookmark-add" @submit.prevent="save">
      <label for="rb-bookmark-label" class="shrink-0 text-muted">{{ t('bookmarks.add.label') }}</label>
      <UInput
        id="rb-bookmark-label"
        v-model="label"
        type="text"
        size="sm"
        class="w-36 shrink-0"
        autocomplete="off"
        data-testid="bookmark-add-label"
      />
      <label for="rb-bookmark-target" class="shrink-0 text-muted">{{ t('bookmarks.add.target') }}</label>
      <UInput
        id="rb-bookmark-target"
        v-model="target"
        type="text"
        size="sm"
        class="w-56 shrink-0"
        :ui="{ base: 'font-mono' }"
        autocomplete="off"
        spellcheck="false"
        :color="invalid ? 'error' : 'primary'"
        :highlight="invalid"
        :aria-invalid="invalid ? 'true' : undefined"
        aria-describedby="rb-bookmark-target-hint"
        data-testid="bookmark-add-target"
        @input="invalid = false"
      />
      <span id="rb-bookmark-target-hint" class="shrink-0 text-xs" :class="invalid ? 'text-error' : 'text-muted'">
        {{ invalid ? t('bookmarks.add.invalid') : t('bookmarks.add.hint') }}
      </span>
      <USwitch
        v-model="follow"
        size="sm"
        class="shrink-0"
        :label="t('bookmarks.add.follow')"
        :title="t('bookmarks.add.follow_hint')"
        :ui="{ label: 'text-muted font-normal' }"
        data-testid="bookmark-add-follow"
      />
      <label for="rb-bookmark-group" class="shrink-0 text-muted">{{ t('bookmarks.add.group') }}</label>
      <UInput
        id="rb-bookmark-group"
        v-model="group"
        type="text"
        size="sm"
        class="w-28 shrink-0"
        autocomplete="off"
        data-testid="bookmark-add-group"
      />
      <USwitch
        v-model="share"
        size="sm"
        class="shrink-0"
        :label="t('bookmarks.add.share')"
        :ui="{ label: 'text-muted font-normal' }"
        data-testid="bookmark-add-scope"
      />
      <UButton type="submit" size="sm" class="shrink-0" data-testid="bookmark-add-save">
        {{ t('bookmarks.add.save') }}
      </UButton>
    </form>
    <UButton size="sm" color="neutral" variant="ghost" class="ml-auto shrink-0" data-testid="bookmark-cancel" @click="close">
      {{ t('bookmarks.cancel') }}
    </UButton>
  </div>
</template>
