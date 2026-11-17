<script setup lang="ts">
// The one tab row of the pages window (RB-080-81): every `UTabs` of the settings - a category's
// sections, the shortcut groups, the plugin tabs - and of About goes through it. Nuxt UI places
// the marker of the chosen tab by its horizontal offset alone, over the full height of the list,
// so a list that wraps into a second row draws the marker across both rows over another tab.
// Here the tabs stay in one row that scrolls sideways: a narrow window shows a back and a
// forward button on the side with more tabs, and the chosen tab - by click, by the arrow keys,
// by a deep link - is scrolled into sight. Wide, every tab fits and the buttons stay away.
import { computed, nextTick, onMounted, onUnmounted, ref, watch, type ComponentPublicInstance } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import UTabs from '@nuxt/ui/components/Tabs.vue';
import { SETTINGS_ICONS } from './icons';
import { overflowEdges, pageOffset, revealOffset, type Edges } from './tabScroll';

export interface TabStripItem {
  label: string;
  value: string;
  /** A count beside the label (the plugin tabs, RB-080-80). */
  badge?: number;
}

defineOptions({ inheritAttrs: false });

const props = withDefaults(defineProps<{ items: TabStripItem[]; variant?: 'pill' | 'link' }>(), { variant: 'pill' });
const model = defineModel<string | number | undefined>();
const { t } = useI18n();

// The neutral pill (dark on light, light on dark) keeps 4.5:1 where the teal accent with white
// text does not; inactive tabs take the toned text and every tab a solid focus ring (WCAG AA).
// `shrink-0`: a tab keeps its label whole, the row scrolls instead.
const TRIGGER =
  'shrink-0 data-[state=inactive]:text-toned focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary';
// One row, scrolled by the buttons, the arrow keys, a wheel with Shift or a swipe; the bar
// itself is hidden - the buttons say there is more. Never a vertical scroll (owner, 2026-11-16,
// RB-080-88: a bar with arrows beside every tab row on Windows): `overflow-x-auto` alone makes
// the other axis `auto` too, and the link variant's line hangs a pixel below the list, so the
// list hides the vertical overflow. The list's own padding (`p-1`) holds the 4 px focus ring.
const LIST =
  'flex-nowrap overflow-x-auto overflow-y-hidden overscroll-x-contain [scrollbar-width:none] [&::-webkit-scrollbar]:hidden';
/** The link variant draws its line inside the list, where the hidden overflow cannot cut it. */
const LINK = {
  list: 'border-b-0 mb-0 shadow-[inset_0_-1px_0_var(--ui-border)]',
  indicator: 'bottom-0',
  trigger: 'after:-bottom-1',
};
const tabsUi = computed(() =>
  props.variant === 'link'
    ? { list: `${LIST} ${LINK.list}`, indicator: LINK.indicator, trigger: `${TRIGGER} ${LINK.trigger}` }
    : { list: LIST, trigger: TRIGGER },
);

const tabs = ref<ComponentPublicInstance | null>(null);
const edges = ref<Edges>({ start: false, end: false });
const overflowing = computed(() => edges.value.start || edges.value.end);
let observer: ResizeObserver | null = null;
let observed: HTMLElement | null = null;

function list(): HTMLElement | null {
  const root = tabs.value?.$el as HTMLElement | undefined;
  return root?.querySelector<HTMLElement>('[role="tablist"]') ?? null;
}

function measure(): void {
  const row = list();
  if (row !== null) {
    edges.value = overflowEdges(row.scrollLeft, row.scrollWidth, row.clientWidth);
  }
}

/** Scrolls the chosen tab into sight; the page itself never scrolls for it. */
function reveal(): void {
  const row = list();
  const active = row?.querySelector<HTMLElement>('[role="tab"][data-state="active"]');
  if (row === null || active === null || active === undefined) {
    return;
  }
  row.scrollLeft = revealOffset(
    { left: active.offsetLeft, width: active.offsetWidth },
    row.scrollLeft,
    row.clientWidth,
    row.scrollWidth,
  );
  measure();
}

function page(direction: 1 | -1): void {
  const row = list();
  if (row !== null) {
    row.scrollLeft = pageOffset(row.scrollLeft, row.clientWidth, row.scrollWidth, direction);
    measure();
  }
}

function attach(): void {
  const row = list();
  if (row === observed) {
    return;
  }
  observed?.removeEventListener('scroll', measure);
  observer?.disconnect();
  observed = row;
  if (row === null) {
    return;
  }
  row.addEventListener('scroll', measure, { passive: true });
  if (typeof ResizeObserver === 'function') {
    observer = new ResizeObserver(reveal);
    observer.observe(row);
  }
}

onMounted(() => {
  attach();
  reveal();
});
onUnmounted(() => {
  observed?.removeEventListener('scroll', measure);
  observer?.disconnect();
});
watch(
  () => [model.value, props.items.length],
  async () => {
    await nextTick();
    attach();
    reveal();
  },
);
</script>

<template>
  <div class="flex w-full min-w-0 items-center gap-1" data-testid="tab-strip">
    <UButton
      v-if="overflowing"
      :icon="SETTINGS_ICONS.back"
      color="neutral"
      variant="ghost"
      size="xs"
      square
      tabindex="-1"
      :class="edges.start ? '' : 'invisible'"
      :aria-label="t('settings.tabs.back')"
      :title="t('settings.tabs.back')"
      data-testid="tabs-back"
      @click="page(-1)"
    />
    <UTabs
      ref="tabs"
      v-bind="$attrs"
      v-model="model"
      :items="items"
      :content="false"
      :variant="variant"
      color="neutral"
      size="sm"
      class="min-w-0 flex-1"
      :ui="tabsUi"
    />
    <UButton
      v-if="overflowing"
      :icon="SETTINGS_ICONS.expand"
      color="neutral"
      variant="ghost"
      size="xs"
      square
      tabindex="-1"
      :class="edges.end ? '' : 'invisible'"
      :aria-label="t('settings.tabs.forward')"
      :title="t('settings.tabs.forward')"
      data-testid="tabs-forward"
      @click="page(1)"
    />
  </div>
</template>
