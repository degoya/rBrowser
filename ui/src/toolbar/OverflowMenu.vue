<script setup lang="ts">
// The toolbar's main menu `≡` (RB-090-63, formerly the overflow menu of RB-080-52): always in the
// row, at its right end. A button whose menu the core draws natively below it (RB-090-45,
// `nativeMenu.ts`), in rBrowser's theme like every toolbar pulldown.
//
// On top what left the row, read from the hidden groups (`overflowEntries.ts`) when the menu
// opens and kept until it closed, so the entry that runs is one that was shown: a switch or a
// toggle as a check row, a hidden pulldown's rows under its name with the one in effect checked.
// Below them the command palette, the bookmarks (directly, the owner's word) and a submenu per
// area with every function of rBrowser (`menu/mainMenu.ts`), each row a registry command with its
// shortcut beside it. `ui.toolbar.overflow.open` (`Leader P T`) opens it; the focus comes back to
// the button when it closed.
//
// A hidden group that tells something (a recording, paused sync, a restricted workspace) puts a
// mark on the button, and its text is in the menu.
import { computed, onScopeDispose, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import type { NativeMenuRow } from '../ipc/protocol';
import { ICONS } from '../icons';
import { useShell } from '../state/shell';
import { windowEvents, type Subscribe } from '../whichkey/whichkey';
import type { ShellToCore } from '../ipc/protocol';
import { MAIN_MENU } from '../menu/mainMenu';
import { buildMenu } from '../menu/menuModel';
import { useMenuContext } from '../menu/useMenuContext';
import { NOTICE_GROUPS, OPEN_COMMAND } from './groups';
import { MAX_ROWS, capRows, isOpeningKey, useNativeMenu } from './nativeMenu';
import { overflowEntries, sections, type OverflowEntry } from './overflowEntries';
import { useToolbarGroup } from './useToolbarFit';

const props = withDefaults(defineProps<{ subscribe?: Subscribe }>(), { subscribe: undefined });
const shell = useShell();
const fit = useToolbarGroup();
const { t } = useI18n();
const menu = useNativeMenu('menu', {
  send: shell.send,
  subscribe: props.subscribe,
  more: (count) => t('shell.menu.more', { count }),
});
const context = useMenuContext(shell);
/** The prefix of the main menu's own rows, apart from the overflowed entries' keys. */
const MAIN = 'main:';

/** The row that counts what a menu of more than the core's rows leaves out; never chosen. */
const MORE = 'more';

const anchor = ref<HTMLElement | null>(null);
/** A group that tells something is in the menu. */
const notice = computed(() => fit.layout.value.hidden.some((group) => NOTICE_GROUPS.includes(group)));
const label = computed(() =>
  notice.value ? t('shell.menu.label_notice') : t('shell.menu.label'),
);

/** Reads the entries from the hidden groups as they are now; the menu is about to open. */
function prepare(): OverflowEntry[] {
  fit.unpin();
  const read = overflowEntries(fit.hiddenGroups());
  // What a control opens beside itself needs its group in the row; where the row has no room
  // for the group beside the address bar, the entry says so instead of opening nothing.
  return read.map((entry) =>
    entry.opens && !entry.disabled && !fit.canPin(entry.group)
      ? { ...entry, disabled: true, label: t('shell.toolbar.overflow.no_room', { label: entry.label }) }
      : entry,
  );
}

/** The entries as rows (at most `max`): a block per heading under it, a line between two blocks. */
function rowsOf(entries: readonly OverflowEntry[], max: number): NativeMenuRow[] {
  const rows: NativeMenuRow[] = [];
  for (const block of sections(entries)) {
    if (rows.length > 0) {
      rows.push({ kind: 'separator' });
    }
    if (block.heading !== null) {
      rows.push({ kind: 'heading', label: block.heading });
    }
    for (const entry of block.entries) {
      const checked = entry.checked === undefined ? {} : { checked: entry.checked };
      const radio = entry.radio === true ? { radio: true } : {};
      rows.push({ kind: 'item', id: entry.key, label: entry.label, disabled: entry.disabled, ...checked, ...radio });
    }
  }
  return capRows(rows, max, (count) => ({
    kind: 'item',
    id: MORE,
    label: t('shell.toolbar.overflow.more', { count }),
    disabled: true,
  }));
}

/** Runs the entry `key` of those the menu showed. */
function run(entries: readonly OverflowEntry[], key: string): void {
  const entry = entries.find((candidate) => candidate.key === key && !candidate.disabled);
  if (entry === undefined) {
    return;
  }
  if (entry.opens) {
    // What the control opens stands beside it: its group comes back into the row first.
    fit.pin(entry.group, entry.run);
  } else {
    entry.run();
  }
}

function open(): void {
  const element = anchor.value;
  if (element === null) {
    return;
  }
  const entries = prepare();
  const main = buildMenu(MAIN_MENU, context(), MAIN);
  // The overflowed entries and the main menu's top level share the core's bound on one menu.
  const overflowed = rowsOf(entries, MAX_ROWS - main.rows.length - 1);
  const rows = overflowed.length === 0 ? main.rows : [...overflowed, { kind: 'separator' as const }, ...main.rows];
  void menu.open(element, rows, (key) => choose(entries, main.actions, key));
}

/** Runs the row `key`: a main menu row's message, else the overflowed entry shown. */
function choose(entries: readonly OverflowEntry[], actions: ReadonlyMap<string, ShellToCore>, key: string): void {
  const message = actions.get(key);
  if (message !== undefined) {
    void shell.send(message);
  } else {
    run(entries, key);
  }
}

function onKeyDown(event: KeyboardEvent): void {
  if (isOpeningKey(event)) {
    event.preventDefault();
    open();
  }
}

/** The command: the menu opens with the focus on its button. */
function command(): void {
  const button = anchor.value?.querySelector('button');
  if (button === null || button === undefined) {
    return;
  }
  button.focus();
  open();
}

const stop = (props.subscribe ?? windowEvents)((message) => {
  if (message.type === 'uiCommand' && message.id === OPEN_COMMAND) {
    command();
  }
});
onScopeDispose(stop);
</script>

<template>
  <div ref="anchor" class="relative ml-auto flex shrink-0 items-center" data-testid="toolbar-overflow">
    <UButton
      :icon="ICONS.menu"
      color="neutral"
      variant="ghost"
      size="sm"
      square
      class="size-8 justify-center"
      aria-haspopup="menu"
      :aria-label="label"
      :title="label"
      data-testid="toolbar-overflow-button"
      @click="open"
      @keydown="onKeyDown"
    />
    <span
      v-if="notice"
      class="pointer-events-none absolute top-0.5 right-0.5 size-2 rounded-full bg-warning"
      data-testid="toolbar-overflow-mark"
    />
  </div>
</template>
