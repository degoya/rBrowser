<script setup lang="ts">
// The overlay's colours (RB-090-58): one field per element - ruler bar, ticks and numbers, guides,
// columns, column edges, baseline grid -, each applied on its own as the person's colour
// (`set` with `colors`), which every view takes at once. Reset brings back the shared settings.
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import ColorField from './ColorField.vue';
import { colorArg, COLOR_KEYS, type ColorKey } from './colors';
import type { RulersState } from './model';

defineProps<{ state: RulersState }>();
const emit = defineEmits<{ change: [args: Record<string, unknown>] }>();
const { t } = usePanel();

const name = (key: ColorKey): string => t(`panel.color_${key}`);
</script>

<template>
  <section v-if="state.colors" id="colors" aria-labelledby="colors-title" aria-describedby="colors-hint" class="flex flex-col gap-3">
    <h2 id="colors-title" class="text-sm font-semibold text-highlighted">{{ t('panel.colors') }}</h2>
    <p id="colors-hint" class="text-xs text-muted">{{ t('panel.colors_hint') }}</p>
    <div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
      <ColorField
        v-for="key in COLOR_KEYS"
        :key="key"
        :value="state.colors[key]"
        :color-key="key"
        :name="name(key)"
        :pick-label="t('panel.color_pick', { name: name(key) })"
        :hex-label="t('panel.color_hex', { name: name(key) })"
        :opacity-label="t('panel.color_opacity', { name: name(key) })"
        :invalid-text="t('panel.color_invalid')"
        @update="(value: string) => emit('change', colorArg(key, value))"
      />
    </div>
  </section>
</template>
