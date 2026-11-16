<script setup lang="ts">
// The one tab row of a plugin panel (RB-090-48), the panels' twin of the shell's `TabStrip.vue`
// (RB-080-81). `UTabs` with `overflow-x-auto` alone makes `overflow-y` auto as well, and the
// `link` marker, one pixel below the row, then gave Windows a vertical scroll bar with arrows
// beside the tabs. Here the row scrolls sideways only, its scroll bar hidden, the link line and
// marker drawn inside it: a narrow panel or a large text size shows a back and a forward button
// on the side with more tabs, standing over the row's ends, and the chosen tab - by click, by
// the arrow keys - is scrolled into sight. Wide, every tab fits and the buttons stay away.
// A drop-in for `UTabs`: the attributes (`id`, `content`, `unmount-on-hide`, `size`, ...) and
// the slots (each item's content) go through; the buttons' texts are the host's.
import { computed, inject, nextTick, onMounted, onUnmounted, ref, watch, type ComponentPublicInstance } from 'vue';
import type { TabsItem } from '@nuxt/ui';
import { PANEL_KEY } from '../mount';
import { fitsWithout, overflowEdges, pageOffset, REVEAL_MARGIN, revealOffset, type Edges } from '../tabScroll';

type Slot = 'root' | 'list' | 'indicator' | 'trigger' | 'content';

defineOptions({ inheritAttrs: false });

const props = withDefaults(defineProps<{ items: TabsItem[]; variant?: 'pill' | 'link'; ui?: Partial<Record<Slot, string>> }>(), {
  variant: 'pill',
  ui: () => ({}),
});
const model = defineModel<string | number | undefined>();

/** English when the panels window sent no text (a test, an older host). */
const BACK = 'Show earlier tabs';
const FORWARD = 'Show more tabs';
const panel = inject(PANEL_KEY, null);
const backText = computed(() => panel?.strings.value['plugins.panel.tabs_back'] ?? BACK);
const forwardText = computed(() => panel?.strings.value['plugins.panel.tabs_forward'] ?? FORWARD);

// One row, scrolled by the buttons, the arrow keys, a wheel with Shift or a swipe; never up and
// down, and no bar in either direction - the buttons say there is more.
const LIST = 'flex-nowrap overflow-x-auto overflow-y-hidden overscroll-x-contain [scrollbar-width:none] [&::-webkit-scrollbar]:hidden';
// The `link` variant draws its line as the row's bottom border and its marker one pixel below
// the row (`-bottom-px`), where a row that clips cuts it off: the line becomes an inset shadow
// and the marker sits on it, both inside.
const LINE = 'shadow-[inset_0_-1px_0_var(--ui-border)]';
const LINK_LIST = `border-b-0 mb-0 ${LINE}`;
// The buttons stand over the row's ends on the row's own ground.
const GROUND = { link: `bg-default ${LINE}`, pill: 'bg-elevated hover:bg-accented' } as const;

const join = (...parts: (string | undefined)[]): string => parts.filter(Boolean).join(' ');
const merged = computed(() => ({
  ...props.ui,
  root: join('min-w-0 max-w-full', props.ui.root),
  list: join(LIST, props.variant === 'link' ? LINK_LIST : undefined, props.ui.list),
  indicator: join(props.variant === 'link' ? 'bottom-0' : undefined, props.ui.indicator),
  // A tab keeps its label whole; the row scrolls instead.
  trigger: join('shrink-0', props.ui.trigger),
}));

const tabs = ref<ComponentPublicInstance | null>(null);
const back = ref<ComponentPublicInstance | null>(null);
const forward = ref<ComponentPublicInstance | null>(null);
const edges = ref<Edges>({ start: false, end: false });
const overflowing = ref(false);
let observer: ResizeObserver | null = null;
let observed: HTMLElement | null = null;

function list(): HTMLElement | null {
  const root = tabs.value?.$el as HTMLElement | undefined;
  return root?.querySelector<HTMLElement>('[role="tablist"]') ?? null;
}

const widthOf = (button: ComponentPublicInstance | null): number => (button?.$el as HTMLElement | undefined)?.offsetWidth ?? 0;

function measure(): void {
  const row = list();
  if (row !== null) {
    overflowing.value = !fitsWithout(row.scrollWidth, row.clientWidth, widthOf(back.value) + widthOf(forward.value));
    edges.value = overflowEdges(row.scrollLeft, row.scrollWidth, row.clientWidth);
  }
}

/** Scrolls the chosen tab into sight, clear of the button over that end; the page never scrolls
 * for it. */
function reveal(): void {
  const row = list();
  const active = row?.querySelector<HTMLElement>('[role="tab"][data-state="active"]');
  if (row === null || active === null || active === undefined) {
    measure();
    return;
  }
  row.scrollLeft = revealOffset(
    { left: active.offsetLeft, width: active.offsetWidth },
    row.scrollLeft,
    row.clientWidth,
    row.scrollWidth,
    REVEAL_MARGIN + Math.max(widthOf(back.value), widthOf(forward.value)),
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
// Another tab, other tabs or other texts (they arrive after the first paint); the buttons that
// came or went change the row as well.
watch([model, () => props.items, overflowing], async () => {
  await nextTick();
  attach();
  reveal();
});
</script>

<template>
  <UTabs ref="tabs" v-bind="$attrs" v-model="model" :items="items" :variant="variant" :ui="merged">
    <template #list-leading>
      <UButton
        v-if="overflowing"
        ref="back"
        icon="i-lucide-chevron-left"
        color="neutral"
        variant="ghost"
        size="xs"
        square
        tabindex="-1"
        aria-hidden="true"
        :title="backText"
        class="sticky start-0 z-10 shrink-0 self-center"
        :class="[GROUND[variant], edges.start ? '' : 'invisible']"
        data-testid="tabs-back"
        @click="page(-1)"
      />
    </template>
    <template #list-trailing>
      <UButton
        v-if="overflowing"
        ref="forward"
        icon="i-lucide-chevron-right"
        color="neutral"
        variant="ghost"
        size="xs"
        square
        tabindex="-1"
        aria-hidden="true"
        :title="forwardText"
        class="sticky end-0 z-10 shrink-0 self-center"
        :class="[GROUND[variant], edges.end ? '' : 'invisible']"
        data-testid="tabs-forward"
        @click="page(1)"
      />
    </template>
    <template v-for="(_, name) in $slots" #[name]="scope">
      <slot :name="name" v-bind="scope ?? {}" />
    </template>
  </UTabs>
</template>
