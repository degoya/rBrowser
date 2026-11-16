<script setup lang="ts">
// One todo file (RB-110-13): its path and counts, the new-todo field with where it goes (the top
// level, a project or a Markdown heading, a choice), its lines and the Archive project folded
// below. The place chosen is the list's filter as well (RB-110-16): it shows that section alone,
// says so and clears with one click; a text written as a heading (`## Sprint`) adds that heading,
// which is chosen next. A file that does not read says why in one sentence and offers nothing to
// change.
import { computed, nextTick, ref, watch } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { addTargets, visible, type Done, type Item, type PanelSettings, type TodoFile } from './model';
import TodoRow from './TodoRow.vue';

const props = defineProps<{ file: TodoFile; settings: PanelSettings; tag: string | null; busy: boolean }>();
const emit = defineEmits<{
  act: [tool: string, args: Record<string, unknown>, done?: Done];
  editing: [on: boolean];
}>();

const panel = usePanel();
const t = panel.t;

const text = ref('');
const target = ref(0);
const archiveOpen = ref(false);
const input = ref<{ inputRef?: HTMLInputElement } | null>(null);

const shown = computed(() => visible(props.file.items, props.settings, props.tag, target.value));
const targets = computed(() => [
  { value: 0, label: t('panel.topLevel') },
  ...addTargets(props.file).map((project) => ({ value: project.line, label: project.label })),
]);
/** The section the list is cut to: its label, `null` for the whole file. */
const section = computed(() => (target.value > 0 ? (targets.value.find((place) => place.value === target.value)?.label ?? null) : null));

// The place follows its line when lines above it came or went (found again by its label); one the
// file no longer has (deleted, or changed elsewhere) gives the whole file again.
watch(targets, (now, before) => {
  if (target.value === 0) return;
  // A place chosen just now (a new heading) was not there before: it stays while the file has it.
  const label = before?.find((place) => place.value === target.value)?.label;
  if (now.some((place) => place.value === target.value && (label === undefined || place.label === label))) return;
  target.value = now.find((place) => place.value !== 0 && place.label === label)?.value ?? 0;
});

/** A heading line as Markdown writes one: one to six `#`, then white space and a text. */
const HEADING = /^#{1,6}[ \t]+\S/;
const failure = computed(() => (props.file.error ? panel.error(Object.assign(new Error(props.file.error.code), { params: props.file.error.params })) : null));

/** How many lines below `item` belong to it: the deeper ones right after it. */
function below(item: Item): number {
  const items = props.file.items;
  const at = items.indexOf(item);
  let count = 0;
  for (const next of items.slice(at + 1)) {
    if (next.depth <= item.depth) break;
    count += 1;
  }
  return count;
}

function add(): void {
  const value = text.value.trim();
  if (value === '') return;
  const args: Record<string, unknown> = { text: value };
  if (target.value > 0) args.under = target.value;
  // A new heading is chosen next, so the todos written after it go below it.
  const done: Done | undefined = HEADING.test(value)
    ? (answer) => {
        if (typeof answer.line === 'number') target.value = answer.line;
      }
    : undefined;
  emit('act', 'add', args, done);
  text.value = '';
}

/** Puts the caret into the new-todo field (the command "Add a todo"). */
async function focusNew(): Promise<void> {
  await nextTick();
  const field = input.value?.inputRef;
  field?.scrollIntoView({ block: 'nearest' });
  field?.focus();
}

defineExpose({ focusNew });
</script>

<template>
  <section :data-file="file.path" class="flex flex-col gap-2" :aria-labelledby="`file-${file.path}`">
    <header class="flex flex-wrap items-center gap-2">
      <UIcon name="i-lucide-list-todo" class="size-4 shrink-0 text-muted" />
      <h2 :id="`file-${file.path}`" class="min-w-0 flex-1 truncate font-mono text-sm text-highlighted">{{ file.path }}</h2>
      <UBadge v-if="!file.error" data-counts color="neutral" variant="subtle" size="sm" :label="t('panel.counts', { open: file.counts.open, done: file.counts.done + file.counts.cancelled })" />
    </header>

    <UAlert v-if="failure" data-error color="error" variant="subtle" icon="i-lucide-circle-x" :title="failure" :ui="{ root: 'px-3 py-2', title: 'text-sm font-normal' }" />

    <template v-else>
      <div class="flex flex-wrap items-center gap-1.5">
        <USelect v-if="targets.length > 1" v-model="target" data-target :items="targets" size="sm" :aria-label="t('panel.addTo')" class="w-40" />
        <UInput
          ref="input"
          v-model="text"
          data-new
          size="sm"
          class="min-w-0 flex-1"
          maxlength="1000"
          autocomplete="off"
          :placeholder="t('panel.addPlaceholder')"
          :aria-label="t('panel.addPlaceholder')"
          @keydown.enter.prevent="add"
        />
        <UButton data-add size="sm" icon="i-lucide-plus" :label="t('panel.add')" :disabled="busy || text.trim() === ''" @click="add" />
      </div>
      <div v-if="section !== null" class="flex">
        <UButton
          data-section-clear
          size="xs"
          color="primary"
          variant="soft"
          trailing-icon="i-lucide-x"
          class="max-w-full"
          :label="t('panel.section', { name: section })"
          :aria-label="t('panel.sectionClear', { name: section })"
          :ui="{ label: 'truncate' }"
          @click="target = 0"
        />
      </div>

      <ul data-items class="flex flex-col gap-1">
        <TodoRow
          v-for="item in shown.active"
          :key="item.line"
          :item="item"
          :below="below(item)"
          :busy="busy"
          @toggle="emit('act', 'toggle', { line: item.line })"
          @cancel="emit('act', 'toggle', { line: item.line, state: item.state === 'cancelled' ? 'open' : 'cancelled' })"
          @save="(value: string) => emit('act', 'edit', { line: item.line, action: 'text', text: value })"
          @remove="emit('act', 'edit', { line: item.line, action: 'delete' })"
          @move="(up: boolean) => emit('act', 'edit', { line: item.line, action: up ? 'up' : 'down' })"
          @editing="(on: boolean) => emit('editing', on)"
        />
      </ul>

      <UCollapsible v-if="shown.archive.length > 0" v-model:open="archiveOpen" :unmount-on-hide="false">
        <UButton
          data-archive-toggle
          size="xs"
          color="neutral"
          variant="ghost"
          :icon="archiveOpen ? 'i-lucide-chevron-down' : 'i-lucide-chevron-right'"
          :label="t('panel.archiveSection', { n: shown.archive.length })"
        />
        <template #content>
          <ul data-archive class="mt-1 flex flex-col gap-1">
            <TodoRow
              v-for="item in shown.archive"
              :key="item.line"
              :item="item"
              :below="below(item)"
              :busy="busy"
              @toggle="emit('act', 'toggle', { line: item.line })"
              @cancel="emit('act', 'toggle', { line: item.line, state: item.state === 'cancelled' ? 'open' : 'cancelled' })"
              @save="(value: string) => emit('act', 'edit', { line: item.line, action: 'text', text: value })"
              @remove="emit('act', 'edit', { line: item.line, action: 'delete' })"
              @move="(up: boolean) => emit('act', 'edit', { line: item.line, action: up ? 'up' : 'down' })"
              @editing="(on: boolean) => emit('editing', on)"
            />
          </ul>
        </template>
      </UCollapsible>
    </template>
  </section>
</template>
