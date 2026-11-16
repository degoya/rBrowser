<script setup lang="ts">
// A button whose tooltip names its shortcut (docs/design.md 5.4, RB-080-86): Nuxt UI's tooltip
// with the keys as `UKbd`, the name as the accessible name, the key in `aria-keyshortcuts`.
// IconAction (panel-ui) has no keys in its tooltip, hence this one; the keys show at any width
// (Nuxt UI hides them below `lg`, and a panel is narrow). `pressed` makes it a toggle
// (`aria-pressed`, solid while on); `primary` and `text` show the name too.
defineOptions({ inheritAttrs: false });
const props = defineProps<{
  icon: string;
  label: string;
  /** The keys as the tooltip shows them, e.g. ['Ctrl', 'Z']. */
  keys?: string[];
  /** The `aria-keyshortcuts` value, e.g. `Control+Z`. */
  shortcut?: string;
  pressed?: boolean;
  primary?: boolean;
  text?: boolean;
  disabled?: boolean;
}>();
defineEmits<{ click: [] }>();

const on = (): boolean => props.primary === true || props.pressed === true;
</script>

<template>
  <UTooltip :text="label" :kbds="keys" :ui="{ kbds: 'inline-flex' }">
    <UButton
      v-bind="$attrs"
      :icon="icon"
      :label="primary || text ? label : undefined"
      :aria-label="label"
      :aria-keyshortcuts="shortcut"
      :aria-pressed="pressed === undefined ? undefined : String(pressed)"
      :color="on() ? 'primary' : 'neutral'"
      :variant="on() ? 'solid' : text ? 'outline' : 'ghost'"
      size="sm"
      :disabled="disabled"
      :data-primary="primary ? '' : undefined"
      @click="$emit('click')"
    />
  </UTooltip>
</template>
