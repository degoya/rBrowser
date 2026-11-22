<script setup lang="ts">
// A text of the panel that names the private values (RB-094-27): the text as it is, then "Open
// Variables", a link that shows the Variables tab here - they are edited there, not in the
// settings window.
import { inject } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { SHOW_SIDE_TAB } from './sideTabs';

defineProps<{ text: string; params?: Record<string, string | number> }>();
const t = usePanel().t;
const show = inject(SHOW_SIDE_TAB, () => undefined);
</script>

<template>
  <span data-variables-text>
    {{ t(text, params ?? {}) }}
    <UButton
      variant="link"
      color="neutral"
      size="xs"
      icon="i-lucide-variable"
      class="px-0 align-baseline"
      data-variables-link
      :label="t('panel.openVariables')"
      @click="show('variables')"
    />
  </span>
</template>
