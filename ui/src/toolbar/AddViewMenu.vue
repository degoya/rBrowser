<script setup lang="ts">
// The "+" beside the layout modes (RB-080-41): the add menu - the breakpoints whose views are
// not open, "Add all", "New size..." - for everyone without a right-click. The toolbar is a strip
// 44 px tall, so a menu drawn here would be cut off at its edge: the button names the place
// under itself (`addViewMenu`) and the core shows the same native menu the empty cells and the
// free canvas have, in front of the views (RB-080-46). `view.add` without a breakpoint
// (`Leader V +`, the palette) reaches the shell as `uiCommand` and opens it too.
import { computed, onScopeDispose, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import { ICONS } from '../icons';
import { useShell } from '../state/shell';
import { windowEvents, type Subscribe } from '../whichkey/whichkey';
import { hiddenBreakpoints, OPEN_COMMAND } from './addView';

const props = withDefaults(defineProps<{ subscribe?: Subscribe }>(), { subscribe: undefined });
const shell = useShell();
const { t } = useI18n();

const anchor = ref<HTMLElement | null>(null);
const hidden = computed(() => hiddenBreakpoints(shell.state.value).length);
const title = computed(() =>
  hidden.value === 0 ? t('shell.toolbar.add_view') : t('shell.toolbar.add_view_count', { count: hidden.value }),
);

/** Asks for the menu with its top-left corner under the button's left end. */
function open(): void {
  const box = anchor.value?.getBoundingClientRect();
  void shell.send({ type: 'addViewMenu', x: Math.round(box?.left ?? 0), y: Math.round(box?.bottom ?? 0) });
}

const stop = (props.subscribe ?? windowEvents)((message) => {
  if (message.type === 'uiCommand' && message.id === OPEN_COMMAND) {
    open();
  }
});
onScopeDispose(stop);
</script>

<template>
  <span ref="anchor" class="inline-flex">
    <UButton
      :icon="ICONS.addView"
      color="neutral"
      variant="ghost"
      size="sm"
      square
      aria-haspopup="menu"
      :aria-label="title"
      :title="title"
      data-testid="add-view"
      :data-hidden="hidden"
      @click="open"
    />
  </span>
</template>
