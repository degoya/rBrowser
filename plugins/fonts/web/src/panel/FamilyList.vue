<script setup lang="ts">
// The Fonts panel's families (RB-080-83): a keyboard list box - arrows, Home and End move,
// Enter or Space marks the family in every view, a click selects and marks. Each family shows
// a line set in its own font (or why there is none), its source, file size, elements, what was
// drawn instead and its findings as badges, and its use per view as a badge per view.
import { nextTick, watch } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { type Answer, type Row, viewLabel } from './model';
import { type FontPreviews, mainFace } from './preview';

const props = defineProps<{ answer: Answer; rows: Row[]; selected: number; previews: FontPreviews }>();
const emit = defineEmits<{ select: [index: number]; mark: [] }>();
const { t, settings } = usePanel();

const familyOf = (name: string) => props.answer.families.find((one) => one.family === name);
const sample = (): string => String(settings.value['fonts.previewText'] ?? '') || 'Aa Bb Cc 0123';

function preview(name: string) {
  const one = familyOf(name);
  return one ? props.previews.state(one, mainFace(one)) : undefined;
}

function onKey(event: KeyboardEvent): void {
  if (event.key === 'Enter' || event.key === ' ') {
    event.preventDefault();
    emit('mark');
    return;
  }
  const moves: Record<string, number> = { ArrowDown: props.selected + 1, ArrowUp: props.selected - 1, Home: 0, End: Number.MAX_SAFE_INTEGER };
  const target = moves[event.key];
  if (target === undefined) return;
  event.preventDefault();
  emit('select', target);
}

function click(index: number): void {
  emit('select', index);
  emit('mark');
}

watch(
  () => props.selected,
  (index) => void nextTick(() => document.getElementById(`family-${index}`)?.scrollIntoView?.({ block: 'nearest' })),
);
</script>

<template>
  <ul
    id="families"
    role="listbox"
    tabindex="0"
    :aria-label="t('panel.families')"
    :aria-activedescendant="rows.length ? `family-${selected}` : undefined"
    class="flex max-h-[70dvh] flex-col gap-1.5 overflow-auto rounded-md outline-none focus-visible:ring-2 focus-visible:ring-primary"
    @keydown="onKey"
  >
    <li
      v-for="(row, index) in rows"
      :id="`family-${index}`"
      :key="row.family"
      role="option"
      :aria-selected="index === selected"
      class="cursor-pointer rounded-md border p-2"
      :class="index === selected ? 'border-primary bg-primary/10' : 'border-default hover:bg-elevated/50'"
      @click="click(index)"
    >
      <div data-name class="truncate text-sm font-semibold text-highlighted">{{ row.family }}</div>
      <div
        v-if="preview(row.family)?.kind === 'ready'"
        data-preview
        class="truncate text-lg leading-snug"
        :style="{ fontFamily: (preview(row.family) as { css: string }).css }"
        aria-hidden="true"
      >
        {{ sample() }}
      </div>
      <div v-else-if="preview(row.family)?.kind === 'missing'" data-preview-missing class="truncate text-xs text-muted">
        {{ t('panel.preview.' + (preview(row.family) as { reason: string }).reason, { kb: (preview(row.family) as { kb?: number }).kb ?? '' }) }}
      </div>
      <div class="mt-1 flex flex-wrap gap-1" data-badges>
        <UBadge color="neutral" variant="subtle" size="sm" icon="i-lucide-globe" :label="t(`panel.provider.${row.provider}`)" />
        <UBadge v-if="familyOf(row.family)?.fileKb != null" color="neutral" variant="outline" size="sm" :label="t('panel.kb', { kb: familyOf(row.family)?.fileKb ?? 0 })" />
        <UBadge v-if="row.elements > 0" color="neutral" variant="subtle" size="sm" :label="t('panel.elementCount', { count: row.elements })" />
        <UBadge v-if="row.rendered === 'fallback' || row.rendered === 'glyphs'" color="warning" variant="subtle" size="sm" :label="t(`panel.badge.${row.rendered}`)" />
        <UBadge v-if="row.findings > 0" color="warning" variant="subtle" size="sm" icon="i-lucide-triangle-alert" :label="t('panel.findingCount', { count: row.findings })" />
      </div>
      <div v-if="familyOf(row.family)?.usage.length" class="mt-1 flex flex-wrap gap-1" data-per-view :aria-label="t('panel.perView')">
        <UBadge
          v-for="use in familyOf(row.family)?.usage ?? []"
          :key="use.view"
          color="neutral"
          variant="outline"
          size="sm"
          :label="`${viewLabel(answer, use.view)} ${use.elements}`"
        />
      </div>
    </li>
  </ul>
</template>
