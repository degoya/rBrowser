<script setup lang="ts">
// The collections (RB-096-01, RB-096-11): the project's requests below `.rbrowser/api/`, by
// folder - an empty collection too -, found by title, URL or method; a file that does not read
// is listed with its reason. A click opens a request; a right-click, the menu key or Shift+F10
// opens the context menu of the row (`menus.ts`), a click on a collection's row too. One
// `UContextMenu` serves the whole tree: the row the menu was asked on picks its items.
import { computed, ref } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import { folderName, treeOf } from './collections';
import { type TreeAction, folderMenu, requestMenu } from './menus';
import type { Listed } from './model';

const props = defineProps<{ requests: Listed[]; folders: string[]; selected: string | null }>();
const emit = defineEmits<{ act: [action: TreeAction] }>();
const t = usePanel().t;

const query = ref('');
const tree = computed(() => treeOf(props.requests, props.folders, query.value));
const target = ref<{ id: string } | { folder: string }>({ folder: '' });
const act = (action: TreeAction): void => emit('act', action);
const items = computed(() => ('id' in target.value ? requestMenu(target.value.id, t, act) : folderMenu(target.value.folder, t, act)));

/** The row a menu is asked on, before the menu opens. */
function aim(event: Event): void {
  const row = (event.target as HTMLElement | null)?.closest<HTMLElement>('[data-id], [data-folder-row]');
  const id = row?.dataset.id;
  target.value = id !== undefined ? { id } : { folder: row?.dataset.folderRow ?? '' };
}

/** The menu of `element`'s row, at its lower left, as a right-click there would open it. */
function menuAt(element: HTMLElement): void {
  const box = element.getBoundingClientRect();
  element.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: box.left + 8, clientY: box.bottom }));
}

function onKey(event: KeyboardEvent): void {
  if (event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10')) {
    event.preventDefault();
    menuAt(event.target as HTMLElement);
  }
}
</script>

<template>
  <div class="flex flex-col gap-2">
    <div class="flex items-center gap-1">
      <UInput id="filter" v-model="query" type="search" icon="i-lucide-search" size="sm" class="min-w-0 flex-1" :placeholder="t('panel.filter')" :aria-label="t('panel.filter')" />
      <IconAction id="new-folder" icon="i-lucide-folder-plus" :label="t('panel.menu.newFolder')" @click="act({ kind: 'newFolder', folder: '' })" />
    </div>
    <UEmpty v-if="requests.length === 0 && folders.length === 0" id="no-requests" icon="i-lucide-folder-open" :title="t('panel.noRequests')" :description="t('panel.noRequestsHint')" variant="naked" />
    <UContextMenu :items="items" size="sm">
      <nav id="collections" class="flex min-h-8 flex-col gap-0.5" :aria-label="t('panel.collections')" @contextmenu.capture="aim" @keydown="onKey">
        <div v-for="folder in tree" :key="folder.folder" :data-folder="folder.folder" class="flex flex-col gap-0.5" :style="{ paddingInlineStart: `${Math.max(folder.depth - 1, 0) * 0.75}rem` }">
          <UButton
            v-if="folder.folder"
            :data-folder-row="folder.folder"
            size="xs"
            color="neutral"
            variant="ghost"
            icon="i-lucide-folder"
            class="justify-start text-muted"
            :aria-label="t('panel.collectionMenu', { folder: folder.folder })"
            aria-haspopup="menu"
            @click="(event: MouseEvent) => menuAt(event.currentTarget as HTMLElement)"
          >
            <span class="min-w-0 truncate">{{ folderName(folder.folder) }}</span>
          </UButton>
          <UButton
            v-for="request in folder.requests"
            :key="request.id"
            :data-id="request.id"
            size="sm"
            color="neutral"
            :variant="request.id === selected ? 'soft' : 'ghost'"
            class="justify-start font-normal"
            :class="folder.folder ? 'ms-3' : ''"
            :aria-current="request.id === selected ? 'true' : undefined"
            @click="act({ kind: 'open', id: request.id })"
          >
            <span class="w-14 shrink-0 font-mono text-xs text-muted">{{ request.method }}</span>
            <span class="min-w-0 truncate">{{ request.name || request.id }}</span>
            <UIcon v-if="request.problem" name="i-lucide-triangle-alert" class="size-3.5 text-warning" :aria-label="t('panel.problem', { problem: request.problem })" />
          </UButton>
        </div>
      </nav>
    </UContextMenu>
  </div>
</template>
