<script setup lang="ts">
// One view's images (RB-093-02): the page and its density above, then a row per image - its file
// (a button that marks it in every view, the keyboard's way to what the hover does), where the
// file came from and its loading flags; the size it is drawn at, the device pixels that needs, the
// file's own pixels and their ratio; its problems as sentences with a severity badge. An image
// not measured says why in place of empty sizes. Hovering a row marks the image in every view.
import { computed } from 'vue';
import type { TableRow } from '@nuxt/ui';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { issueText, num, type Row, rowsOf, SEVERITY_COLOR, SEVERITY_ICON, text, type ViewAnswer } from './model';

const props = defineProps<{ view: ViewAnswer; onlyProblems: boolean }>();
const emit = defineEmits<{ mark: [selector: string | null] }>();
const { strings } = usePanel();

const rows = computed(() => rowsOf(strings.value, props.view, props.onlyProblems));
const columns = computed(() => [
  { id: 'image', header: text(strings.value, 'panel.table.image') },
  { accessorKey: 'rendered', header: text(strings.value, 'panel.table.rendered'), meta: { class: { td: 'tabular-nums whitespace-nowrap align-top' } } },
  { accessorKey: 'needed', header: text(strings.value, 'panel.table.needed'), meta: { class: { td: 'tabular-nums whitespace-nowrap align-top' } } },
  { accessorKey: 'file', header: text(strings.value, 'panel.table.file'), meta: { class: { td: 'tabular-nums whitespace-nowrap align-top' } } },
  { accessorKey: 'ratio', header: text(strings.value, 'panel.table.ratio'), meta: { class: { td: 'tabular-nums align-top' } } },
  { id: 'problems', header: text(strings.value, 'panel.table.problems') },
]);

function hover(_event: Event, row: TableRow<Row> | null): void {
  emit('mark', row ? row.original.image.selector : null);
}
</script>

<template>
  <section class="flex flex-col gap-2 pt-2">
    <p v-if="view.url" data-page class="text-xs break-all text-muted">
      {{ text(strings, 'panel.page', { url: view.url, width: view.width, height: view.height, dpr: num(view.dpr) }) }}
    </p>
    <p v-if="view.lcpKnown === false" data-lcp-guessed class="flex items-center gap-1.5 text-xs text-muted">
      <UIcon name="i-lucide-info" class="size-4 shrink-0" />
      <span>{{ text(strings, 'panel.lcpGuessed') }}</span>
    </p>
    <UEmpty v-if="(view.images ?? []).length === 0" data-empty icon="i-lucide-image-off" :title="text(strings, 'panel.empty')" variant="naked" />
    <p v-else-if="rows.length === 0" data-no-problems class="flex items-center gap-1.5 text-sm text-muted">
      <UIcon name="i-lucide-circle-check" class="size-4 shrink-0 text-success" />
      <span>{{ text(strings, 'panel.noProblems') }}</span>
    </p>
    <UTable v-else :data="rows" :columns="columns" :on-hover="hover" class="text-sm" @mouseleave="emit('mark', null)">
      <template #image-cell="{ row }">
        <div class="flex min-w-0 flex-col items-start gap-1">
          <UButton
            data-image
            color="neutral"
            variant="link"
            class="h-auto max-w-56 p-0 text-left break-all whitespace-normal"
            :label="row.original.name"
            :aria-label="text(strings, 'panel.mark', { name: row.original.name })"
            @click="emit('mark', row.original.image.selector)"
            @focus="emit('mark', row.original.image.selector)"
            @blur="emit('mark', null)"
          />
          <span class="text-xs text-muted">{{ row.original.origin }}</span>
          <div v-if="row.original.flags.length > 0 || row.original.note" class="flex flex-wrap gap-1">
            <UBadge v-for="flag in row.original.flags" :key="flag" data-flag color="neutral" variant="outline" size="sm" :label="flag" />
            <UBadge v-if="row.original.note" data-note color="neutral" variant="subtle" size="sm" :label="row.original.note" />
          </div>
        </div>
      </template>
      <template #problems-cell="{ row }">
        <ul class="flex flex-col gap-1.5">
          <li v-for="issue in row.original.image.issues" :key="issue.message" data-issue class="flex items-start gap-1.5 whitespace-normal">
            <UBadge :color="SEVERITY_COLOR[issue.severity]" variant="subtle" size="sm" :icon="SEVERITY_ICON[issue.severity]" :label="text(strings, `panel.severity.${issue.severity}`)" />
            <span>{{ issueText(strings, issue) }}</span>
          </li>
        </ul>
      </template>
    </UTable>
    <p v-if="(view.more ?? 0) > 0" data-more class="text-xs text-muted">{{ text(strings, 'panel.more', { count: view.more ?? 0 }) }}</p>
    <p v-if="(view.hidden ?? 0) > 0" data-hidden class="text-xs text-muted">{{ text(strings, 'panel.hidden', { count: view.hidden ?? 0 }) }}</p>
  </section>
</template>
