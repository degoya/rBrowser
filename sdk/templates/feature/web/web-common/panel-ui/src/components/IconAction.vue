<script setup lang="ts">
// A header action (RB-080-83, docs/design.md 5.4): an icon button with its name as the accessible
// name and the tooltip; the main action of a panel shows its text too (`primary`).
// The attributes (an `id`) go to the button, not to the tooltip.
defineOptions({ inheritAttrs: false });
defineProps<{ icon: string; label: string; primary?: boolean; disabled?: boolean }>();
defineEmits<{ click: [] }>();
</script>

<template>
  <UTooltip :text="label">
    <UButton
      v-bind="$attrs"
      :icon="icon"
      :label="primary ? label : undefined"
      :aria-label="label"
      :color="primary ? 'primary' : 'neutral'"
      :variant="primary ? 'solid' : 'ghost'"
      size="sm"
      :disabled="disabled"
      :data-primary="primary ? '' : undefined"
      @click="$emit('click')"
    />
  </UTooltip>
</template>
