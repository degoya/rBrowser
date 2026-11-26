<script setup lang="ts">
// The page tree of the sitemap start page (RB-099-09): one row per site, folder and page, with a
// checkbox that chooses the node's whole subtree (a dash when only part of it is chosen) and the
// number of pages below a folder. A tree of our own on Nuxt UI parts (UCheckbox, UButton): UTree
// keeps its selection as a list of items and walks it for every row, too slow for 10,000 pages.
// Only the rows on screen are drawn (`windowOf`); a node's state is two prefix sums (`tree.ts`).
// Keyboard, as a WAI-ARIA tree with one tab stop (`aria-activedescendant`): Up/Down, Home/End and
// Page Up/Down move; Right opens a folder or goes to its first child; Left closes it or goes to
// its parent; Space chooses or lets go of the row's pages; Enter opens or closes a folder.
// A row's choice is its `aria-checked`; the active row is only the `aria-activedescendant`, no
// `aria-selected` beside it, so a screen reader says one state, not two (RB-099-21).
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import UCheckbox from '@nuxt/ui/components/Checkbox.vue';
import { START_ICONS } from './icons';
import {
  checkState,
  countsOf,
  nodeCounts,
  toggle,
  visibleRows,
  windowOf,
  type FlatNode,
  type PageTree,
  type Scope,
  type Selection,
} from './tree';

/** A row's height in px (`h-7`). */
const ROW = 28;
/** The viewport before it is measured (and in a test, where nothing has a size). */
const FALLBACK_VIEWPORT = 480;

const props = defineProps<{
  tree: PageTree;
  selection: Selection;
  scope: Scope;
  /** Raised by the page after every change of the selection. */
  version: number;
}>();
const emit = defineEmits<{ change: [] }>();
const { t } = useI18n();

/** The open folders; the sites are open at first. */
const opened = ref(new Set<number>(props.tree.roots));
/** Under a search every folder is open, unless it was closed while searching. */
const closedInSearch = ref(new Set<number>());
const active = ref(props.tree.roots[0] ?? -1);
const scrollTop = ref(0);
const viewport = ref(FALLBACK_VIEWPORT);
const list = ref<HTMLElement | null>(null);

const counts = computed(() => {
  void props.version;
  return countsOf(props.selection, props.scope);
});

function isOpen(id: number): boolean {
  return props.scope === null ? opened.value.has(id) : !closedInSearch.value.has(id);
}

const rows = computed(() => visibleRows(props.tree, isOpen, counts.value));

function nodeOf(id: number): FlatNode {
  const node = props.tree.nodes[id];
  if (node === undefined) throw new Error(`no tree node ${id}`);
  return node;
}

/** The in-scope siblings of `node`: its position and their number (`aria-posinset`, `-setsize`). */
function position(node: FlatNode): { at: number; of: number } {
  const siblings = node.parent < 0 ? props.tree.roots : nodeOf(node.parent).children;
  const kept = siblings.filter((id) => nodeCounts(nodeOf(id), counts.value).inScope > 0);
  return { at: kept.indexOf(node.id) + 1, of: kept.length };
}

const shown = computed(() => {
  const { start, end } = windowOf(scrollTop.value, viewport.value, ROW, rows.value.length);
  return rows.value.slice(start, end).map((id, at) => {
    const node = nodeOf(id);
    const { inScope } = nodeCounts(node, counts.value);
    const state = checkState(node, counts.value);
    return { id, node, top: (start + at) * ROW, inScope, state, folder: node.children.length > 0, ...position(node) };
  });
});

// The active row stays one that is shown (a search may hide it).
watch(rows, (now) => {
  if (!now.includes(active.value)) active.value = now[0] ?? -1;
});

function ariaChecked(state: boolean | 'indeterminate'): 'true' | 'false' | 'mixed' {
  return state === 'indeterminate' ? 'mixed' : state ? 'true' : 'false';
}

/** A page's row names its URL, a folder its name. */
function titleOf(node: FlatNode): string {
  return node.children.length === 0 && node.count === 1 ? (props.tree.pages[node.first] ?? node.name) : node.name;
}

function choose(id: number): void {
  toggle(nodeOf(id), props.selection, props.scope, counts.value);
  emit('change');
}

function setOpen(id: number, open: boolean): void {
  if (props.scope === null) {
    if (open) opened.value.add(id);
    else opened.value.delete(id);
  } else if (open) {
    closedInSearch.value.delete(id);
  } else {
    closedInSearch.value.add(id);
  }
}

function flip(id: number): void {
  setOpen(id, !isOpen(id));
}

/** Scrolls row `index` into the viewport. */
function reveal(index: number): void {
  const element = list.value;
  if (element === null) return;
  const top = index * ROW;
  if (top < element.scrollTop) element.scrollTop = top;
  else if (top + ROW > element.scrollTop + viewport.value) element.scrollTop = top + ROW - viewport.value;
  scrollTop.value = element.scrollTop;
}

function moveTo(index: number): void {
  const last = rows.value.length - 1;
  if (last < 0) return;
  const at = Math.min(Math.max(index, 0), last);
  active.value = rows.value[at] ?? active.value;
  reveal(at);
}

function onKey(event: KeyboardEvent): void {
  const at = rows.value.indexOf(active.value);
  const node = props.tree.nodes[active.value];
  const page = Math.max(1, Math.floor(viewport.value / ROW) - 1);
  const folder = node !== undefined && node.children.length > 0;
  switch (event.key) {
    case 'ArrowDown':
      moveTo(at + 1);
      break;
    case 'ArrowUp':
      moveTo(at - 1);
      break;
    case 'Home':
      moveTo(0);
      break;
    case 'End':
      moveTo(rows.value.length - 1);
      break;
    case 'PageDown':
      moveTo(at + page);
      break;
    case 'PageUp':
      moveTo(at - page);
      break;
    case 'ArrowRight':
      if (folder && !isOpen(active.value)) setOpen(active.value, true);
      else if (folder) moveTo(at + 1);
      break;
    case 'ArrowLeft':
      if (folder && isOpen(active.value)) setOpen(active.value, false);
      else if (node !== undefined && node.parent >= 0) moveTo(rows.value.indexOf(node.parent));
      break;
    case ' ':
      if (node !== undefined) choose(node.id);
      break;
    case 'Enter':
      if (folder) flip(active.value);
      break;
    default:
      return;
  }
  event.preventDefault();
}

function onScroll(): void {
  scrollTop.value = list.value?.scrollTop ?? 0;
}

let resize: ResizeObserver | null = null;
onMounted(() => {
  const element = list.value;
  if (element === null) return;
  const measure = (): void => {
    viewport.value = element.clientHeight > 0 ? element.clientHeight : FALLBACK_VIEWPORT;
  };
  measure();
  if (typeof ResizeObserver !== 'undefined') {
    resize = new ResizeObserver(measure);
    resize.observe(element);
  }
});
onBeforeUnmount(() => resize?.disconnect());

defineExpose({ focus: () => list.value?.focus() });
</script>

<template>
  <div
    ref="list"
    role="tree"
    tabindex="0"
    :aria-label="t('sitemap.start.tree_label')"
    :aria-activedescendant="active >= 0 ? `sitemap-node-${active}` : undefined"
    class="relative min-h-0 flex-1 overflow-auto rounded-md bg-elevated focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary"
    data-testid="sitemap-tree"
    @keydown="onKey"
    @scroll="onScroll"
  >
    <p v-if="rows.length === 0" class="p-3 text-muted" data-testid="sitemap-tree-empty">
      {{ t('sitemap.start.no_match') }}
    </p>
    <div class="relative" :style="{ height: `${rows.length * ROW}px` }" role="presentation">
      <div
        v-for="row in shown"
        :id="`sitemap-node-${row.id}`"
        :key="row.id"
        role="treeitem"
        :aria-level="row.node.level"
        :aria-posinset="row.at"
        :aria-setsize="row.of"
        :aria-expanded="row.folder ? isOpen(row.id) : undefined"
        :aria-checked="ariaChecked(row.state)"
        :title="titleOf(row.node)"
        class="absolute inset-x-0 flex h-7 items-center gap-1 pr-2 text-sm"
        :class="row.id === active ? 'bg-accented text-highlighted' : 'text-default'"
        :style="{ top: `${row.top}px`, paddingLeft: `${(row.node.level - 1) * 1.25 + 0.25}rem` }"
        :data-testid="`node-${row.id}`"
        @click="active = row.id"
      >
        <UButton
          v-if="row.folder"
          size="xs"
          color="neutral"
          variant="ghost"
          tabindex="-1"
          aria-hidden="true"
          :icon="isOpen(row.id) ? START_ICONS.startFolderOpen : START_ICONS.startFolderClosed"
          :data-testid="`node-${row.id}-flip`"
          @click.stop="flip(row.id)"
        />
        <span v-else class="inline-block w-6 shrink-0" />
        <UCheckbox
          :model-value="row.state"
          tabindex="-1"
          aria-hidden="true"
          :data-testid="`node-${row.id}-check`"
          @update:model-value="choose(row.id)"
          @click.stop="active = row.id"
        />
        <span class="min-w-0 truncate" data-name>{{ row.node.name }}</span>
        <span v-if="row.folder" class="ml-auto shrink-0 tabular-nums text-muted">
          {{ t('sitemap.start.node_pages', { count: row.inScope }) }}
        </span>
      </div>
    </div>
  </div>
</template>
