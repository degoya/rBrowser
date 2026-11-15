<script setup lang="ts">
// The toolbar's overflow menu (RB-080-52): what left the row, by name. A native select like the
// toolbar's other lists (RB-080-16) - CEF opens it as a native popup in front of the views, where
// an HTML menu would be cut off at the edge of the 44 px strip.
//
// Its entries are read from the hidden groups (`overflowEntries.ts`) only when the menu is about
// to open - a press, a key, the command - and never while its popup is open: the popup takes
// pointer and keys itself, so the entry that runs is one that was shown.
//
// An entry runs only when it is chosen: a key that browses the closed select opens its popup or
// runs nothing (`menuKeys.ts`). `ui.toolbar.overflow.open` (`Leader P T`) gives the select the focus;
// the command itself is no gesture, so the first key opens the list.
//
// A hidden group that tells something (a recording, paused sync, a restricted workspace) puts a
// mark on the button, and its text is in the menu.
import { computed, onScopeDispose, ref, shallowRef, useId } from 'vue';
import { useI18n } from 'vue-i18n';
import UIcon from '@nuxt/ui/components/Icon.vue';
import { ICONS } from '../icons';
import { useShell } from '../state/shell';
import { windowEvents, type Subscribe } from '../whichkey/whichkey';
import { NOTICE_GROUPS, OPEN_COMMAND } from './groups';
import { createMenuKeys, openPicker } from './menuKeys';
import { overflowEntries, sections, type OverflowEntry } from './overflowEntries';
import { useToolbarGroup } from './useToolbarFit';

const props = withDefaults(defineProps<{ subscribe?: Subscribe }>(), { subscribe: undefined });
const shell = useShell();
const fit = useToolbarGroup();
const { t } = useI18n();
const id = useId();

const select = ref<HTMLSelectElement | null>(null);
const entries = shallowRef<OverflowEntry[]>([]);
const blocks = computed(() => sections(entries.value));
const shown = computed(() => fit.layout.value.menu);
/** A group that tells something is in the menu. */
const notice = computed(() => fit.layout.value.hidden.some((group) => NOTICE_GROUPS.includes(group)));
const label = computed(() =>
  notice.value ? t('shell.toolbar.overflow.label_notice') : t('shell.toolbar.overflow.label'),
);
/** Reads the entries from the hidden groups as they are now; the menu is about to open. */
function prepare(): void {
  fit.unpin();
  const read = overflowEntries(fit.hiddenGroups(), {
    on: (name) => t('shell.toolbar.overflow.on', { label: name }),
    off: (name) => t('shell.toolbar.overflow.off', { label: name }),
  });
  // What a control opens beside itself needs its group in the row; where the row has no room
  // for the group beside the address bar, the entry says so instead of opening nothing.
  entries.value = read.map((entry) =>
    entry.opens && !entry.disabled && !fit.canPin(entry.group)
      ? { ...entry, disabled: true, label: t('shell.toolbar.overflow.no_room', { label: entry.label }) }
      : entry,
  );
}

/** Browsing keys open the list or run nothing (`menuKeys.ts`). */
const keys = createMenuKeys(prepare);

function onChange(event: Event): void {
  const element = event.target as HTMLSelectElement;
  const chosen = element.value;
  // The select names the menu, not an entry: it goes back to its label.
  element.value = '';
  if (keys.browsing()) {
    return;
  }
  const entry = entries.value.find((candidate) => candidate.key === chosen && !candidate.disabled);
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

/** The command: the menu takes the focus; with nothing in it, the status bar says so. */
function open(): void {
  const element = select.value;
  if (!shown.value || element === null) {
    shell.report({ key: 'shell.toolbar.overflow.nothing', args: {}, level: 'info' });
    return;
  }
  keys.release();
  prepare();
  element.focus();
  // Works where the command came with a gesture; else the first key opens the list.
  openPicker(element);
}

const stop = (props.subscribe ?? windowEvents)((message) => {
  if (message.type === 'uiCommand' && message.id === OPEN_COMMAND) {
    open();
  }
});
onScopeDispose(stop);
</script>

<template>
  <div v-if="shown" class="relative ml-auto flex shrink-0 items-center" data-testid="toolbar-overflow">
    <label :for="id" class="sr-only">{{ label }}</label>
    <select
      :id="id"
      ref="select"
      :title="label"
      value=""
      class="h-8 w-8 appearance-none rounded-md bg-default pr-0 pl-8 text-sm text-default ring ring-inset ring-accented focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary"
      data-testid="toolbar-overflow-select"
      @pointerdown="keys.onPointerDown"
      @keydown="keys.onKeyDown"
      @keyup="keys.release"
      @blur="keys.release"
      @change="onChange"
    >
      <option value="" disabled selected>{{ label }}</option>
      <template v-for="block in blocks" :key="block.entries[0]?.key">
        <optgroup v-if="block.heading !== null" :label="block.heading">
          <option v-for="entry in block.entries" :key="entry.key" :value="entry.key" :disabled="entry.disabled">
            {{ entry.label }}
          </option>
        </optgroup>
        <template v-else>
          <option v-for="entry in block.entries" :key="entry.key" :value="entry.key" :disabled="entry.disabled">
            {{ entry.label }}
          </option>
        </template>
      </template>
    </select>
    <UIcon :name="ICONS.more" class="pointer-events-none absolute top-1/2 left-2 size-4 -translate-y-1/2" />
    <span
      v-if="notice"
      class="pointer-events-none absolute top-0.5 right-0.5 size-2 rounded-full bg-warning"
      data-testid="toolbar-overflow-mark"
    />
  </div>
</template>
