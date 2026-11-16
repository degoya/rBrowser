<script setup lang="ts">
// The row header of the storage grid (RB-070-01, on Nuxt UI since RB-080-84): the name, where
// it lives (origin or domain and path), `pinned` and the row's state (differs, missing) as
// badges - the state in words, not by colour alone. Column 0 of the grid's keyboard model.
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import type { StorageRow } from './model';

defineProps<{ row: StorageRow; index: number; focused: boolean }>();
const { t } = usePanel();
</script>

<template>
  <div
    data-cell
    data-name
    role="rowheader"
    :data-row="index"
    data-col="0"
    :tabindex="focused ? 0 : -1"
    class="-mx-2 -my-1.5 flex min-w-32 max-w-64 cursor-pointer flex-col gap-0.5 px-2 py-1.5 outline-none focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-inset"
  >
    <span class="flex min-w-0 items-center gap-1.5">
      <span data-key class="truncate font-mono text-xs font-semibold text-highlighted">{{ row.name }}</span>
      <UBadge v-if="row.pinned" data-pin color="primary" variant="subtle" size="sm" icon="i-lucide-pin" :label="t('panel.pinned')" />
    </span>
    <span data-where class="truncate text-xs text-muted">{{ row.scope + (row.path ?? '') }}</span>
    <UBadge
      v-if="row.state !== 'same'"
      data-state
      :color="row.state === 'missing' ? 'error' : 'warning'"
      variant="subtle"
      size="sm"
      class="self-start"
      :label="t('panel.state.' + row.state)"
    />
  </div>
</template>
