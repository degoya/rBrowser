<script setup lang="ts">
// The command palette (RB-060-11) in its popup: the double leader (`app.command_palette`) makes
// the core show a frameless window of its own right below the toolbar (`settings.html#/palette`,
// ADR 0001: dropdowns as native CEF popups), so the shell strip keeps its 44 px and the grid its
// size. It opens at once on load and afresh on every `uiCommand`. One field, the best matches
// below it - Nuxt UI's `UCommandPalette` (RB-090-46), fed the palette's own ranking as one group
// it does not filter. The focus stays in the field (an ARIA combobox), Up and Down move the
// selection, Page Up and Page Down by ten, Enter runs, Ctrl+Enter opens the entry's command in
// the shortcut editor, Escape leaves a step or closes, Backspace in the empty field leaves a
// step, and the number of matches is announced. Running or leaving it hides the popup
// (`dismissPalette`); losing the focus hides it on the core's side.
// `view.add_size` opens it in the step "New view size ..." (RB-080-46, the popup's first load
// through the route key `initialStep`): the picker of sizes, devices and "Custom...".
import type { CommandPaletteGroup, CommandPaletteItem } from '@nuxt/ui';
import { computed, inject, nextTick, onMounted, onScopeDispose, ref, useId, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import UBadge from '@nuxt/ui/components/Badge.vue';
import UButton from '@nuxt/ui/components/Button.vue';
import UCommandPalette from '@nuxt/ui/components/CommandPalette.vue';
import UKbd from '@nuxt/ui/components/Kbd.vue';
import { ICONS } from '../icons';
import { SETTINGS_ICONS } from '../settings/icons';
import { useShell } from '../state/shell';
import { windowEvents, type Subscribe } from '../whichkey/whichkey';
import { PALETTE_COMMAND } from './constants';
import { STEP_NEW_VIEW } from './types';
import { createPalette, PALETTE_CLIENT_KEY } from './palette';
import { useRowHighlight } from './rowHighlight';

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

/** One row of `UCommandPalette`: an entry's key, title and place in the list. */
interface Row extends CommandPaletteItem {
  key: string;
}

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
const root = ref<HTMLElement | null>(null);
const baseId = useId();
const listId = `${baseId}-list`;
const hintId = `${baseId}-hint`;

/** The core's state has not come yet (the popup's first load): an empty list is no answer. */
const loading = computed(() => shell.state.value === null);
const selected = computed(() => results.value[active.value]);
const entries = computed(() => results.value.slice(0, Math.max(ROWS, active.value + 1)));
const more = computed(() => results.value.length - entries.value.length);
/** The entries by key: a row's group and shortcut (the list's slots see the row only). */
const byKey = computed(() => new Map(entries.value.map((entry) => [entry.key, entry])));
const description = computed(() => (options.value.showDescriptions ? selected.value?.description : undefined));

const groups = computed<CommandPaletteGroup<Row>[]>(() => [
  {
    id: 'matches',
    // The palette ranks the entries itself (`rank.ts`): recent first, steps, prefixes.
    ignoreFilter: true,
    items: entries.value.map((entry, index): Row => ({
      key: entry.key,
      label: entry.title,
      'aria-setsize': results.value.length,
      'aria-posinset': index + 1,
      onSelect: (event: Event) => {
        // The palette runs it; the list keeps no value of its own.
        event.preventDefault();
        void palette.choose(entry);
      },
    })),
  },
]);

const placeholder = computed(() => {
  if (step.value !== null) {
    return t('palette.input');
  }
  return options.value.prefixes ? t('palette.placeholder') : t('palette.placeholder_plain');
});

/** The field: a combobox over the list, described by the hint; the palette sees its blur. */
const input = computed(() => ({
  role: 'combobox',
  'aria-autocomplete': 'list',
  'aria-expanded': 'true',
  'aria-controls': listId,
  'aria-describedby': hintId,
  'aria-label': t('palette.input'),
  autocomplete: 'off',
  spellcheck: 'false',
  onBlur,
}));

const field = (): HTMLInputElement | null => root.value?.querySelector('input') ?? null;
const highlight = useRowHighlight({ root, active, rows: entries, select: (index) => palette.select(index) });

// `UCommandPalette` names its list itself; the field's `aria-controls` needs its id.
watch(
  root,
  (element) => {
    const list = element?.querySelector('[role="listbox"]');
    list?.setAttribute('id', listId);
    list?.setAttribute('aria-label', t('palette.options'));
  },
  { flush: 'post' },
);

function begin(newView = false): void {
  void palette.open(newView ? { id: STEP_NEW_VIEW, title: t('palette.step.new_view') } : null);
  void nextTick(() => field()?.focus());
}

const stop = (props.subscribe ?? windowEvents)((message) => {
  if (message.type === 'uiCommand' && (message.id === PALETTE_COMMAND || message.id === NEW_VIEW_COMMAND)) {
    begin(message.id === NEW_VIEW_COMMAND);
  }
});
onScopeDispose(stop);
onMounted(() => begin(props.initialStep === STEP_NEW_VIEW));

/**
 * The palette's keys, taken in the capture phase before `UCommandPalette` sees them: the
 * selection is the palette's (`palette.ts`), so Page Up and Page Down, the wrap from the first
 * row to the last match and a selection kept across a rebuild hold. Home and End stay the field's.
 */
function onKey(event: KeyboardEvent): void {
  if (event.key === 'Escape') {
    event.preventDefault();
    event.stopPropagation();
    palette.escape();
    return;
  }
  if (event.target !== field() || event.isComposing) {
    return;
  }
  const moves: Record<string, number> = { ArrowDown: 1, ArrowUp: -1, PageDown: PAGE, PageUp: -PAGE };
  const delta = moves[event.key];
  if (delta !== undefined) {
    event.preventDefault();
    event.stopPropagation();
    palette.move(delta);
  } else if (event.key === 'Home' || event.key === 'End') {
    event.stopPropagation();
  } else if (event.key === 'Enter') {
    event.preventDefault();
    event.stopPropagation();
    void (event.ctrlKey ? palette.editShortcut() : palette.choose());
  } else if (event.key === 'Backspace' && query.value === '' && step.value !== null) {
    event.preventDefault();
    event.stopPropagation();
    palette.back();
  }
}

/** A press anywhere but the field keeps the focus there: a click runs a row, the field stays. */
function onPress(event: MouseEvent): void {
  if (event.target !== field()) {
    event.preventDefault();
  }
}

/** The popup lost the focus (a click elsewhere): it hides itself, the focus is where it went. */
function onBlur(event: FocusEvent): void {
  const next = event.relatedTarget;
  if (next instanceof Node && root.value?.contains(next)) {
    return;
  }
  void palette.close(false, false);
}
</script>

<template>
  <div
    v-if="isOpen"
    ref="root"
    class="fixed inset-0 flex flex-col overflow-hidden border border-default bg-default text-sm text-default"
    role="dialog"
    :aria-label="t('palette.label')"
    data-testid="palette"
    @keydown.capture="onKey"
    @mousedown="onPress"
    @pointermove.capture="highlight.onPointerMove"
  >
    <div v-if="step" class="flex shrink-0 items-center gap-1 px-2 pt-2">
      <UButton
        :icon="ICONS.back"
        size="xs"
        color="neutral"
        variant="ghost"
        :aria-label="t('palette.back')"
        :title="t('palette.back')"
        data-testid="palette-back"
        @click="palette.back()"
        @blur="onBlur"
      />
      <UBadge size="md" variant="subtle" class="min-w-0 truncate font-semibold" :label="step.title" data-testid="palette-step" />
    </div>
    <UCommandPalette
      v-model:search-term="query"
      :groups="groups"
      :model-value="selected?.key"
      value-key="key"
      :placeholder="placeholder"
      :icon="SETTINGS_ICONS.search"
      :input="input"
      :autofocus="false"
      :fuse="{ resultLimit: Number.POSITIVE_INFINITY }"
      :ui="{ content: 'min-h-0 flex-1', itemTrailingIcon: 'hidden' }"
      class="min-h-0 flex-1"
      @highlight="highlight.onHighlight"
      @leave="highlight.onLeave"
    >
      <template #item-trailing="{ item }">
        <span v-if="byKey.get(item.key)?.group" class="shrink-0 text-xs text-muted">{{ byKey.get(item.key)?.group }}</span>
        <UKbd v-if="byKey.get(item.key)?.shortcut" :value="byKey.get(item.key)?.shortcut" size="sm" class="shrink-0" />
      </template>
      <template #empty>
        <!-- While the core's state loads the list stays blank; the status names the count. -->
        <span>{{ loading ? '' : t('palette.results', { count: 0 }) }}</span>
      </template>
      <template v-if="more > 0 || description" #footer>
        <p v-if="more > 0" class="px-2 text-xs text-muted" data-testid="palette-more">
          {{ t('palette.more', { count: more }) }}
        </p>
        <p v-if="description" class="truncate px-2 text-xs text-muted" data-testid="palette-description">{{ description }}</p>
      </template>
    </UCommandPalette>
    <span :id="hintId" class="sr-only">{{ step ? t('palette.hint_step') : t('palette.hint') }}</span>
    <span class="sr-only" role="status" aria-live="polite">{{ t('palette.results', { count: results.length }) }}</span>
  </div>
</template>
