<script setup lang="ts">
// A whole-number option of the chosen tool (RB-080-86): its name, Nuxt UI's slider named by it,
// and the value beside it.
const value = defineModel<number>({ required: true });
defineProps<{ id: string; option: string; label: string; range: readonly [number, number] }>();
</script>

<template>
  <span :data-option="option" class="flex items-center gap-2 text-sm">
    <span :id="`${id}-label`">{{ label }}</span>
    <USlider :id="id" v-model="value" :min="range[0]" :max="range[1]" :step="1" size="sm" class="w-28" :aria-labelledby="`${id}-label`" />
    <span class="w-8 text-right tabular-nums text-muted">{{ value }}</span>
  </span>
</template>
