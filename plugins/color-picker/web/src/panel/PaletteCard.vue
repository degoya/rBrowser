<script setup lang="ts">
// The palette of the active view (RB-070-10, on Nuxt UI since RB-080-86): a row per colour by
// use - its swatch (our own, as in ColorSlot), the hex value, how often it is used as a plural -
// and per row Mark and Use as foreground or background as icon buttons named in full.
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import type { PaletteGroup, Slot } from './model';

defineProps<{ groups: PaletteGroup[] }>();
const emit = defineEmits<{ read: []; clear: []; mark: [group: PaletteGroup]; use: [slot: Slot, hex: string] }>();
const { t } = usePanel();

const CARD_UI = { header: 'flex items-center gap-2 px-3 py-2 sm:px-3', body: 'p-3 sm:p-3' };
</script>

<template>
  <UCard as="section" aria-labelledby="palette-title" :ui="CARD_UI">
    <template #header>
      <h2 id="palette-title" class="min-w-0 flex-1 truncate text-sm font-semibold text-highlighted">{{ t('panel.palette') }}</h2>
      <IconAction id="palette-clear" icon="i-lucide-eraser" :label="t('panel.marks_clear')" @click="emit('clear')" />
      <UButton id="palette-read" size="sm" variant="soft" icon="i-lucide-palette" :label="t('panel.palette_read')" @click="emit('read')" />
    </template>
    <UEmpty v-if="groups.length === 0" id="palette-empty" icon="i-lucide-palette" :description="t('panel.palette_empty')" variant="naked" size="sm" />
    <ul v-else id="palette" class="divide-y divide-default">
      <li v-for="group in groups" :key="group.hex" class="flex flex-wrap items-center gap-2 py-1.5">
        <span aria-hidden="true" class="inline-block size-5 shrink-0 rounded border border-accented" :style="{ background: group.hex }" />
        <code class="font-mono text-sm">{{ group.hex }}</code>
        <UBadge data-uses color="neutral" variant="outline" size="sm" :label="t('panel.uses', { count: group.count })" />
        <span class="ms-auto flex items-center gap-0.5">
          <IconAction data-mark icon="i-lucide-scan-search" :label="t('panel.mark_uses', { value: group.hex })" @click="emit('mark', group)" />
          <IconAction data-use="foreground" icon="i-lucide-type" :label="t('panel.use_as_foreground', { value: group.hex })" @click="emit('use', 'foreground', group.hex)" />
          <IconAction data-use="background" icon="i-lucide-paint-bucket" :label="t('panel.use_as_background', { value: group.hex })" @click="emit('use', 'background', group.hex)" />
        </span>
      </li>
    </ul>
  </UCard>
</template>
