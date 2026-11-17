<script setup lang="ts">
// A toolbar split button (RB-090-63): DevTools, Screenshot, Video, Checks. The button runs its
// main command at once; the arrow beside it opens the rest as the core's native menu below it
// (`nativeMenu.ts`), built from the button's menu tree (`menu/buttonMenus.ts`) with check marks,
// radio choices and each command's shortcut. Both halves are native buttons with a name; the
// arrow opens on a click, Enter, Space, the arrow keys and F4. Out of the row the group's rows
// are in the main menu `≡` (`overflowEntries.ts`), as a pulldown's are.
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import UFieldGroup from '@nuxt/ui/components/FieldGroup.vue';
import { ICONS } from '../icons';
import type { NativeMenuRow, ShellToCore } from '../ipc/protocol';
import { buildMenu, commandMessage, type MenuNode } from '../menu/menuModel';
import { useMenuContext } from '../menu/useMenuContext';
import { useShell } from '../state/shell';
import { isOpeningKey, provideMenuSource, useNativeMenu, type PulldownKind } from './nativeMenu';

const props = defineProps<{
  kind: PulldownKind;
  icon: string;
  /** The main button's name (what a click does). */
  label: string;
  /** The arrow's name (what its menu holds). */
  menuLabel: string;
  /** The command a click on the main button runs. */
  command: string;
  menu: readonly MenuNode[];
  /**
   * The state the main button shows (DevTools open: `on`); a button without one is a plain
   * button, no toggle (a string, as Vue would read an absent boolean as `false`).
   */
  state?: 'on' | 'off';
  /** A longer description for the main button's tooltip. */
  title?: string;
}>();

const shell = useShell();
const { t } = useI18n();
const context = useMenuContext(shell);
/** The main button's tooltip: its description and the shortcut of its command. */
const tooltip = computed(() => {
  const text = props.title ?? props.label;
  const shortcut = shell.bindingOf(props.command);
  return shortcut === null ? text : t('shell.menu.button.with_shortcut', { label: text, shortcut });
});
const native = useNativeMenu(props.kind, { send: shell.send, more: (count) => t('shell.menu.more', { count }) });
const anchor = ref<HTMLElement | null>(null);

/** What each row of the menu built last sends. */
let actions = new Map<string, ShellToCore>();

function rows(): NativeMenuRow[] {
  const built = buildMenu(props.menu, context(), `${props.kind}:`);
  actions = built.actions;
  return built.rows;
}

function choose(id: string): void {
  const message = actions.get(id);
  if (message !== undefined) {
    void shell.send(message);
  }
}

function run(): void {
  void shell.send(commandMessage(props.command));
}

function open(): void {
  const element = anchor.value;
  if (element !== null) {
    void native.open(element, rows(), choose);
  }
}

function onKeyDown(event: KeyboardEvent): void {
  if (isOpeningKey(event)) {
    event.preventDefault();
    open();
  }
}

/** The arrow's rows for the main menu while the group is out of the row. */
let stopSource: (() => void) | null = null;
const arrow = ref<{ $el: HTMLElement } | null>(null);
watch(
  arrow,
  (button) => {
    stopSource?.();
    const element = button?.$el ?? null;
    stopSource = element instanceof HTMLButtonElement ? provideMenuSource(element, { rows, choose }) : null;
  },
  { flush: 'post' },
);
onBeforeUnmount(() => stopSource?.());
</script>

<template>
  <span ref="anchor" class="inline-flex shrink-0 items-center" :data-testid="`split-${kind}`">
    <UFieldGroup size="sm">
      <UButton
        :icon="icon"
        color="neutral"
        :variant="state === 'on' ? 'soft' : 'ghost'"
        square
        class="size-8 justify-center"
        :aria-label="label"
        :aria-pressed="state === undefined ? undefined : state === 'on'"
        :title="tooltip"
        :data-testid="`split-${kind}-run`"
        @click="run"
      />
      <UButton
        ref="arrow"
        :icon="ICONS.pulldown"
        color="neutral"
        variant="ghost"
        class="w-4 justify-center px-0"
        aria-haspopup="menu"
        :aria-label="menuLabel"
        :title="menuLabel"
        :data-testid="`split-${kind}-menu`"
        @click="open"
        @keydown="onKeyDown"
      />
    </UFieldGroup>
  </span>
</template>
