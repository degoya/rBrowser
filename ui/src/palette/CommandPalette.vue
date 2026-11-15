<script setup lang="ts">
// The command palette (RB-060-11) in its popup: the double leader (`app.command_palette`) makes
// the core show a frameless window of its own right below the toolbar (`settings.html#/palette`,
// ADR 0001: dropdowns as native CEF popups), so the shell strip keeps its 44 px and the grid its
// size. It opens at once on load and afresh on every `uiCommand`. One field, the best matches
// below it. An ARIA combobox: the focus stays in the field, Up and Down move the selection
// (`aria-activedescendant`), Enter runs, Ctrl+Enter opens the entry's command in the shortcut
// editor, Escape leaves a step or closes, and the number of matches is announced. Running or
// leaving it hides the popup (`dismissPalette`); losing the focus hides it on the core's side.
// `view.add_size` opens it in the step "New view size ..." (RB-080-46, the popup's first load
// through the route key `initialStep`): the picker of sizes, devices and "Custom...".
import { computed, inject, nextTick, onMounted, onScopeDispose, ref, useId, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import UKbd from '@nuxt/ui/components/Kbd.vue';
import { useShell } from '../state/shell';
import { windowEvents, type Subscribe } from '../whichkey/whichkey';
import { PALETTE_COMMAND } from './constants';
import { STEP_NEW_VIEW } from './types';
import { createPalette, PALETTE_CLIENT_KEY } from './palette';

/** How far Page Up and Page Down move the selection. */
const PAGE = 10;
/**
 * The rows the list mounts at least; it scrolls. A short list (a step such as the new-size
 * picker) is complete - ten rows at a time hid its devices and "Custom..." on a page nobody saw
 * (RB-080-68). A long one mounts this many, more as the selection goes further, and names the
 * rest: a bare prefix matches hundreds of entries on every keystroke.
 */
const ROWS = 50;

/** The command that opens the palette in the step "New view size ..." (`toolbar/addView.ts`). */
const NEW_VIEW_COMMAND = 'view.add_size';

const props = withDefaults(defineProps<{ subscribe?: Subscribe; initialStep?: string | null }>(), {
  subscribe: undefined,
  initialStep: null,
});
const shell = useShell();
const client = inject(PALETTE_CLIENT_KEY);
if (client === undefined) {
  throw new Error('CommandPalette needs the IPC client (PALETTE_CLIENT_KEY)');
}
const { t } = useI18n();
const palette = createPalette({
  client,
  send: (message) => shell.send(message),
  dismiss: () => shell.send({ type: 'dismissPalette' }).then(() => undefined),
  state: () => shell.state.value,
  t: (key, args) => (args === undefined ? t(key) : t(key, args)),
});
const { isOpen, query, step, active, options, results } = palette;
const field = ref<HTMLInputElement | null>(null);
const baseId = useId();
const listId = `${baseId}-list`;
const hintId = `${baseId}-hint`;

const list = ref<HTMLElement | null>(null);
const selected = computed(() => results.value[active.value]);
const rows = computed(() => results.value.slice(0, Math.max(ROWS, active.value + 1)));
const more = computed(() => results.value.length - rows.value.length);
const optionId = (index: number): string => `${baseId}-option-${index}`;
const activeId = computed(() => (selected.value === undefined ? undefined : optionId(active.value)));

// The selected row stays in sight while the keys move it through a long list.
watch(activeId, async (id) => {
  await nextTick();
  const row = id === undefined ? null : list.value?.querySelector(`[id="${id}"]`);
  row?.scrollIntoView?.({ block: 'nearest' });
});
const placeholder = computed(() => {
  if (step.value !== null) {
    return t('palette.input');
  }
  return options.value.prefixes ? t('palette.placeholder') : t('palette.placeholder_plain');
});

function begin(newView = false): void {
  void palette.open(newView ? { id: STEP_NEW_VIEW, title: t('palette.step.new_view') } : null);
  void nextTick(() => field.value?.focus());
}

const stop = (props.subscribe ?? windowEvents)((message) => {
  if (message.type === 'uiCommand' && (message.id === PALETTE_COMMAND || message.id === NEW_VIEW_COMMAND)) {
    begin(message.id === NEW_VIEW_COMMAND);
  }
});
onScopeDispose(stop);
onMounted(() => begin(props.initialStep === STEP_NEW_VIEW));

function onKey(event: KeyboardEvent): void {
  const moves: Record<string, number> = { ArrowDown: 1, ArrowUp: -1, PageDown: PAGE, PageUp: -PAGE };
  const delta = moves[event.key];
  if (delta !== undefined) {
    event.preventDefault();
    palette.move(delta);
  } else if (event.key === 'Enter') {
    event.preventDefault();
    void (event.ctrlKey ? palette.editShortcut() : palette.choose());
  } else if (event.key === 'Escape') {
    event.preventDefault();
    event.stopPropagation();
    palette.escape();
  } else if (event.key === 'Backspace' && query.value === '' && step.value !== null) {
    event.preventDefault();
    palette.back();
  }
}

/** The popup lost the focus (a click elsewhere): it hides itself, the focus is where it went. */
function onBlur(): void {
  void palette.close(false, false);
}
</script>

<template>
  <div
    v-if="isOpen"
    class="fixed inset-0 flex flex-col gap-1 overflow-hidden border border-default bg-elevated p-2 text-sm text-default"
    role="dialog"
    :aria-label="t('palette.label')"
    data-testid="palette"
  >
    <div class="flex shrink-0 items-center gap-2">
      <span
        v-if="step"
        class="shrink-0 rounded-md bg-primary/10 px-2 py-1 text-xs font-semibold text-highlighted"
        data-testid="palette-step"
      >{{ step.title }}</span>
      <input
        ref="field"
        v-model="query"
        type="text"
        role="combobox"
        aria-autocomplete="list"
        aria-expanded="true"
        :aria-controls="listId"
        :aria-activedescendant="activeId"
        :aria-describedby="hintId"
        :aria-label="t('palette.input')"
        :placeholder="placeholder"
        autocomplete="off"
        spellcheck="false"
        class="h-8 min-w-0 flex-1 rounded-md bg-default px-2 text-sm text-default ring ring-inset ring-accented focus-visible:outline-2 focus-visible:outline-primary"
        @keydown="onKey"
        @blur="onBlur"
      />
    </div>
    <ul
      :id="listId"
      ref="list"
      role="listbox"
      :aria-label="t('palette.options')"
      class="flex min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto p-0.5"
    >
      <li
        v-for="(entry, index) in rows"
        :id="optionId(index)"
        :key="entry.key"
        role="option"
        :aria-selected="index === active"
        :aria-setsize="results.length"
        :aria-posinset="index + 1"
        class="flex min-w-0 shrink-0 cursor-pointer items-center gap-1.5 rounded-md px-2 py-1"
        :class="index === active ? 'bg-primary/15 text-highlighted outline-2 outline-primary' : 'text-default hover:bg-accented/50'"
        @mousedown.prevent
        @click="palette.choose(entry)"
      >
        <span class="min-w-0 flex-1 truncate">{{ entry.title }}</span>
        <span v-if="entry.group" class="shrink-0 text-xs text-muted">{{ entry.group }}</span>
        <UKbd v-if="entry.shortcut" :value="entry.shortcut" size="sm" class="shrink-0" />
      </li>
    </ul>
    <p v-if="more > 0" class="shrink-0 px-2 text-xs text-muted" data-testid="palette-more">
      {{ t('palette.more', { count: more }) }}
    </p>
    <p
      v-if="options.showDescriptions && selected?.description"
      class="shrink-0 truncate text-xs text-muted"
      data-testid="palette-description"
    >{{ selected.description }}</p>
    <span :id="hintId" class="sr-only">{{ step ? t('palette.hint_step') : t('palette.hint') }}</span>
    <span class="sr-only" role="status" aria-live="polite">{{ t('palette.results', { count: results.length }) }}</span>
  </div>
</template>
