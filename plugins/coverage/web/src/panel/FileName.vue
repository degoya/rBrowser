<script setup lang="ts">
// A file in one line (RB-080-84): its type by content as a badge first (a style sheet served as
// a script module says so in the badge's tooltip), `inline` as a second badge, the name cut with
// the full address in the tooltip.
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import type { FileRow } from './model';

const props = defineProps<{ row: FileRow }>();
const { t } = usePanel();
const viaScript = (): boolean => props.row.kind !== props.row.measured;
</script>

<template>
  <div class="flex min-w-0 items-center gap-1.5" :data-url="row.url" :data-kind="row.kind">
    <UTooltip :text="viaScript() ? t('panel.viaScript') : t('panel.kindName.' + row.kind)">
      <UBadge data-type :color="row.kind === 'css' ? 'info' : 'warning'" variant="subtle" size="sm" class="shrink-0" :label="t('panel.kind.' + row.kind)" />
    </UTooltip>
    <UBadge v-if="row.inline" data-inline color="neutral" variant="outline" size="sm" class="shrink-0" :label="t('panel.inline')" />
    <UTooltip :text="row.url" :content="{ side: 'top', align: 'start' }">
      <span data-name class="min-w-0 truncate font-mono text-xs text-highlighted">{{ row.name }}</span>
    </UTooltip>
  </div>
</template>
