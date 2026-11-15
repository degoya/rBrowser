<script setup lang="ts">
// A file's one value (RB-080-84): unused bytes and share in words, a bar for the share, and when
// the breakpoints differ the span as a badge with what it means in its tooltip.
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { type FileRow, bytes, differs, percent } from './model';

defineProps<{ row: FileRow }>();
const { t } = usePanel();
</script>

<template>
  <div class="flex min-w-36 flex-col items-end gap-1">
    <div class="flex items-center gap-1.5">
      <UTooltip v-if="differs(row)" :text="t('panel.spanTitle', { from: percent(row.percent), to: percent(row.maxPercent) })">
        <UBadge data-span color="neutral" variant="outline" size="sm" icon="i-lucide-split" :label="t('panel.span', { from: percent(row.percent), to: percent(row.maxPercent) })" />
      </UTooltip>
      <span data-value class="text-sm tabular-nums text-highlighted">{{ t('panel.unusedValue', { size: bytes(row.unused), percent: percent(row.percent) }) }}</span>
    </div>
    <UProgress :model-value="row.percent" size="xs" color="warning" class="w-28" :aria-label="t('panel.unused')" />
  </div>
</template>
