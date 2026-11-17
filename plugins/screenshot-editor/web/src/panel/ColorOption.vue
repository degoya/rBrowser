<script setup lang="ts">
// The annotation colour (RB-080-86): a button showing the colour opens a popover with the
// swatches - buttons, so the keyboard picks a colour - and Nuxt UI's colour picker for any other
// by pointer (UColorPicker has no keyboard support of its own).
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { SWATCHES } from './controls';

const color = defineModel<string>({ required: true });
const panel = usePanel();
const t = panel.t;

/** The picker's value as the editor keeps it: `#rrggbb`, lower case. */
function pick(value: string | undefined): void {
  if (value && /^#[0-9a-fA-F]{6}$/.test(value)) color.value = value.toLowerCase();
}
</script>

<template>
  <UPopover :content="{ side: 'bottom', align: 'start' }">
    <UButton id="color" color="neutral" variant="outline" size="sm" :aria-label="`${t('panel.color')}: ${color}`" :data-value="color">
      <span class="size-4 rounded-sm border border-default" :style="{ backgroundColor: color }" />
    </UButton>
    <template #content>
      <div class="flex w-56 flex-col gap-2 p-2" role="group" :aria-label="t('panel.color')">
        <div class="grid grid-cols-8 gap-1">
          <UButton
            v-for="[name, value] in SWATCHES"
            :key="name"
            :data-swatch="name"
            :aria-label="t(`panel.colors.${name}`)"
            :aria-pressed="String(color === value)"
            color="neutral"
            variant="ghost"
            size="xs"
            class="p-0.5"
            :class="color === value ? 'ring-2 ring-primary' : ''"
            @click="color = value"
          >
            <span class="block size-5 rounded-sm border border-default" :style="{ backgroundColor: value }" />
          </UButton>
        </div>
        <UColorPicker :model-value="color" size="sm" @update:model-value="pick" />
        <p class="text-xs text-muted tabular-nums">{{ color }}</p>
      </div>
    </template>
  </UPopover>
</template>
