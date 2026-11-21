<script setup lang="ts">
// The "+" beside the layout modes (RB-080-41): the add menu - the breakpoints whose views are
// not open, "Add all", "New size...", then the devices and engines (RB-094-20) - for everyone
// without a right-click. The toolbar is a strip 44 px tall, so a menu drawn here would be cut
// off at its edge: the rows are built here (`menu/addMenu.ts`) and the core shows them as a
// native menu below the button, in front of the views (`nativeMenu.ts`, kind `add`), and lists
// the devices again for the next opening. `view.add` without a breakpoint (`Leader V +`, the
// palette) reaches the shell as `uiCommand` and opens it too. The empty cells and the free
// canvas keep the core's own add menu.
import { computed, onScopeDispose, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import { ICONS } from '../icons';
import { ADD_MENU } from '../menu/addMenu';
import { buildMenu } from '../menu/menuModel';
import { useMenuContext } from '../menu/useMenuContext';
import { useShell } from '../state/shell';
import { windowEvents, type Subscribe } from '../whichkey/whichkey';
import { hiddenBreakpoints, OPEN_COMMAND } from './addView';
import { isOpeningKey, useNativeMenu } from './nativeMenu';

const props = withDefaults(defineProps<{ subscribe?: Subscribe }>(), { subscribe: undefined });
const shell = useShell();
const { t } = useI18n();
const menu = useNativeMenu('add', {
  send: shell.send,
  subscribe: props.subscribe,
  more: (count) => t('shell.menu.more', { count }),
});
const context = useMenuContext(shell);

const anchor = ref<HTMLElement | null>(null);
const hidden = computed(() => hiddenBreakpoints(shell.state.value).length);
const title = computed(() =>
  hidden.value === 0 ? t('shell.toolbar.add_view') : t('shell.toolbar.add_view_count', { count: hidden.value }),
);

/** Opens the rows as they are now below the button; a choice runs its row's message. */
function open(): void {
  const element = anchor.value;
  if (element === null) {
    return;
  }
  const built = buildMenu(ADD_MENU, context(), 'add:');
  void menu.open(element, built.rows, (id) => {
    const message = built.actions.get(id);
    if (message !== undefined) {
      void shell.send(message);
    }
  });
}

function onKeyDown(event: KeyboardEvent): void {
  if (isOpeningKey(event)) {
    event.preventDefault();
    open();
  }
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
      @keydown="onKeyDown"
    />
  </span>
</template>
