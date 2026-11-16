<script setup lang="ts">
// The toolbar's overflow menu (RB-080-52): what left the row, by name. A "More" button whose menu
// the core draws natively below it (RB-090-45, `nativeMenu.ts`), in rBrowser's theme like every
// toolbar pulldown - a `<select>`'s list was Chromium's popup, which followed neither light nor
// dark; an HTML menu would be cut off at the edge of the 44 px strip.
//
// Its entries are read from the hidden groups (`overflowEntries.ts`) when the menu opens - a
// click, Enter, Space, an arrow key, the command - and kept until it closed, so the entry that runs
// is one that was shown. A hidden pulldown's rows stand under its name, the one in effect checked;
// a line between two blocks. `ui.toolbar.overflow.open` (`Leader P T`) opens it; the focus comes
// back to the button when it closed.
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
import { NOTICE_GROUPS, OPEN_COMMAND } from './groups';
import { MAX_ROWS, capRows, isOpeningKey, useNativeMenu } from './nativeMenu';
import { overflowEntries, sections, type OverflowEntry } from './overflowEntries';
import { useToolbarGroup } from './useToolbarFit';

const props = withDefaults(defineProps<{ subscribe?: Subscribe }>(), { subscribe: undefined });
const shell = useShell();
const fit = useToolbarGroup();
const { t } = useI18n();
const menu = useNativeMenu('overflow', { send: shell.send, subscribe: props.subscribe });

/** The row that counts what a menu of more than the core's rows leaves out; never chosen. */
const MORE = 'more';

const anchor = ref<HTMLElement | null>(null);
const shown = computed(() => fit.layout.value.menu);
/** A group that tells something is in the menu. */
const notice = computed(() => fit.layout.value.hidden.some((group) => NOTICE_GROUPS.includes(group)));
const label = computed(() =>
  notice.value ? t('shell.toolbar.overflow.label_notice') : t('shell.toolbar.overflow.label'),
);

/** Reads the entries from the hidden groups as they are now; the menu is about to open. */
function prepare(): OverflowEntry[] {
  fit.unpin();
  const read = overflowEntries(fit.hiddenGroups(), {
    on: (name) => t('shell.toolbar.overflow.on', { label: name }),
    off: (name) => t('shell.toolbar.overflow.off', { label: name }),
  });
  // What a control opens beside itself needs its group in the row; where the row has no room
  // for the group beside the address bar, the entry says so instead of opening nothing.
  return read.map((entry) =>
    entry.opens && !entry.disabled && !fit.canPin(entry.group)
      ? { ...entry, disabled: true, label: t('shell.toolbar.overflow.no_room', { label: entry.label }) }
      : entry,
  );
}

/** The entries as rows: a block per heading under it, a line between two blocks. */
function rowsOf(entries: readonly OverflowEntry[]): NativeMenuRow[] {
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
      rows.push({ kind: 'item', id: entry.key, label: entry.label, disabled: entry.disabled, ...checked });
    }
  }
  return capRows(rows, MAX_ROWS, (count) => ({
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
  void menu.open(element, rowsOf(entries), (key) => run(entries, key));
}

function onKeyDown(event: KeyboardEvent): void {
  if (isOpeningKey(event)) {
    event.preventDefault();
    open();
  }
}

/** The command: the menu opens with the focus on its button; with nothing in it, the status bar says so. */
function command(): void {
  const button = anchor.value?.querySelector('button');
  if (!shown.value || button === null || button === undefined) {
    shell.report({ key: 'shell.toolbar.overflow.nothing', args: {}, level: 'info' });
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
  <div v-if="shown" ref="anchor" class="relative ml-auto flex shrink-0 items-center" data-testid="toolbar-overflow">
    <UButton
      :icon="ICONS.more"
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
