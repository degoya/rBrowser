<script setup lang="ts">
// The snippets (RB-070-04, on Nuxt UI since RB-080-84): a keyboard list box - arrows, Home and End
// move, Enter or Space opens a snippet in the editor (with Ctrl it runs it too), a click opens it.
// Each shows its name, source, world and timeout, its description, and why it cannot run.
import { nextTick, ref, watch } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { type SnippetEntry, failureText, moveFocus } from './model';

const props = defineProps<{ entries: SnippetEntry[]; selected: SnippetEntry | null }>();
const emit = defineEmits<{ open: [entry: SnippetEntry, run: boolean] }>();
const panel = usePanel();
const t = panel.t;
const focus = ref(0);
const list = ref<HTMLElement | null>(null);

const isSelected = (entry: SnippetEntry): boolean => props.selected?.source === entry.source && props.selected?.name === entry.name;
const rows = (): HTMLElement[] => [...(list.value?.querySelectorAll<HTMLElement>('[role="option"]') ?? [])];

watch(
  () => props.entries,
  (now) => {
    focus.value = Math.min(focus.value, Math.max(now.length - 1, 0));
  },
);

/** Puts the keyboard focus on the first snippet. */
function focusFirst(): void {
  focus.value = 0;
  void nextTick(() => rows()[0]?.focus());
}
defineExpose({ focusFirst, reset: () => (focus.value = 0) });

function onKey(event: KeyboardEvent, index: number): void {
  const moved = moveFocus(index, event.key, props.entries.length);
  if (moved !== null) {
    event.preventDefault();
    focus.value = moved;
    void nextTick(() => rows()[moved]?.focus());
  } else if (event.key === 'Enter' || event.key === ' ') {
    event.preventDefault();
    const entry = props.entries[index];
    if (entry) emit('open', entry, event.ctrlKey || event.metaKey);
  }
}
</script>

<template>
  <ol id="list" ref="list" role="listbox" :aria-label="t('panel.list')" class="flex flex-col gap-1.5">
    <li
      v-for="(entry, index) in entries"
      :key="entry.source + '\u0000' + entry.name"
      role="option"
      :tabindex="index === focus ? 0 : -1"
      :aria-selected="isSelected(entry)"
      :data-index="index"
      class="cursor-pointer rounded-md border px-2 py-1.5 outline-none focus-visible:ring-2 focus-visible:ring-primary"
      :class="isSelected(entry) ? 'border-primary bg-primary/10' : 'border-default hover:bg-elevated/50'"
      @click="emit('open', entry, false)"
      @focus="focus = index"
      @keydown="onKey($event, index)"
    >
      <div class="flex flex-wrap items-center gap-1.5">
        <span data-name class="min-w-0 truncate text-sm font-medium text-highlighted">{{ entry.name }}</span>
        <UBadge data-source :color="entry.source === 'local' ? 'primary' : 'neutral'" variant="subtle" size="sm" :label="t('panel.source.' + entry.source)" />
        <span data-meta class="text-xs text-muted">{{ t('panel.world.' + entry.world) }}, {{ entry.timeoutMs }} ms</span>
      </div>
      <p v-if="entry.description" data-description class="text-xs text-muted">{{ entry.description }}</p>
      <p v-if="entry.error" data-invalid class="flex items-start gap-1 text-xs text-error">
        <UIcon name="i-lucide-circle-x" class="mt-0.5 size-3.5 shrink-0" />
        <span>{{ t('panel.invalid', { reason: failureText(panel.strings.value, entry.error.code, entry.error.params) }) }}</span>
      </p>
    </li>
  </ol>
</template>
