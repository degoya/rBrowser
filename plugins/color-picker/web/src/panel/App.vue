<script setup lang="ts">
// The colour picker panel (RB-070-10), on Nuxt UI since RB-080-86 - its look only; the calls,
// events and keyboard are the old panel's. The two colours of the contrast in every format to
// copy, their WCAG ratio and levels, the palette of the active view and the text measure of every
// view. The command `pick` starts the pipette in the views (`{ stop: true }` ends it), the tools
// `contrast`, `palette` and `measure_text` do the rest; the pipette started anywhere (`picking`),
// ended in a view (`stopped`), a colour taken (`picked`) and a measurement started elsewhere
// (`measured`) arrive as events. While picking, the button is pressed and says how to stop
// (RB-080-64); Esc in the panel ends it too.
import { computed, onBeforeUnmount, reactive, ref } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { onPanelEvent } from '../../../../web-common/panel-ui/src/events';
import type { StatusLine, Tone } from '../../../../web-common/panel-ui/src/status';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import PanelFrame from '../../../../web-common/panel-ui/src/components/PanelFrame.vue';
import ColorSlot from './ColorSlot.vue';
import MeasureCard from './MeasureCard.vue';
import PaletteCard from './PaletteCard.vue';
import { asMeasured, asRating, failureKey, order, SLOTS, type Measured, type PaletteGroup, type Rating, type Slot } from './model';

const PREFIX = 'plugin.color-picker.';
const panel = usePanel();
const t = panel.t;

/** The colour texts the contrast is asked for, as typed or picked. */
const texts = reactive<Record<Slot, string>>({ foreground: '#000000', background: '#ffffff' });
const rating = ref<Rating | null>(null);
const measured = ref<Measured | null>(null);
const palette = ref<PaletteGroup[]>([]);
const selector = ref('');
let selectorTouched = false;
/** The pipette is on in the views: the button is pressed, a click or Esc ends it. */
const picking = ref(false);
const status = ref<StatusLine | null>(null);

const formats = computed(() => order(panel.settings.value['color-picker.defaultFormat']));

function say(text: string, tone: Tone = 'info'): void {
  status.value = { tone, text };
}

/** A failure's sentence: the text of its code (with its reason), the panel's own where the code
 * has none - never the code itself. */
function failure(error: unknown): string {
  const code = error instanceof Error ? error.message : String(error);
  const key = failureKey(code, panel.strings.value, PREFIX);
  if (key === null) return t('panel.failed');
  const params = error instanceof Error ? (error as { params?: Record<string, unknown> }).params : undefined;
  return panel.error(Object.assign(new Error(key), params ? { params } : {}));
}
const failed = (error: unknown): void => say(failure(error), 'error');

function setPicking(on: boolean, said?: string): void {
  picking.value = on;
  if (said) say(t(said));
}

function stopPicking(): void {
  setPicking(false, 'panel.stopped');
  panel.call({ command: 'pick' }, { stop: true }).catch(failed);
}

/** The pick button: starts the pipette, at once pressed; a second click ends it. */
function togglePick(): void {
  if (picking.value) {
    stopPicking();
    return;
  }
  setPicking(true, 'panel.picking');
  panel.call({ command: 'pick' }).catch((error: unknown) => {
    setPicking(false);
    failed(error);
  });
}

function updateContrast(): Promise<void> {
  return panel.call({ tool: 'contrast' }, { ...texts }).then((answer) => {
    rating.value = asRating(answer) ?? rating.value;
  }, failed);
}

function setColor(slot: Slot, value: string): Promise<void> {
  texts[slot] = value;
  return updateContrast();
}

function swap(): void {
  [texts.foreground, texts.background] = [texts.background, texts.foreground];
  void updateContrast();
}

async function copy(value: string, field: HTMLInputElement | null): Promise<void> {
  try {
    await navigator.clipboard.writeText(value);
  } catch {
    // The sandboxed frame may not reach the async clipboard: the old way, on the field.
    field?.focus();
    field?.select();
    if (!document.execCommand('copy')) {
      say(t('panel.copy_failed'), 'error');
      return;
    }
  }
  say(t('panel.copied', { value }), 'ok');
}

function readPalette(): void {
  say(t('panel.reading'), 'busy');
  panel.call({ tool: 'palette' }, {}).then((answer) => {
    const view = (answer as { views?: { label?: string; error?: unknown; colors?: unknown }[] } | null)?.views?.[0];
    if (!view || view.error) {
      say(failure(String(view?.error ?? 'panel.no_view')), 'error');
      return;
    }
    palette.value = Array.isArray(view.colors) ? (view.colors as PaletteGroup[]) : [];
    say(t('panel.palette_read_done', { count: palette.value.length, view: view.label }), 'ok');
  }, failed);
}

function mark(group: PaletteGroup): void {
  panel.call({ tool: 'palette' }, { mark: group.members ?? [] }).then(() => say(t('panel.marked', { value: group.hex }), 'ok'), failed);
}

function clearMarks(tool: 'palette' | 'measure_text'): void {
  const args = tool === 'palette' ? { mark: [] } : { clear: true };
  panel.call({ tool }, args).then(() => say(t('panel.cleared'), 'ok'), failed);
}

function measure(): void {
  const wanted = selector.value.trim();
  say(t('panel.measuring'), 'busy');
  panel.call({ tool: 'measure_text' }, wanted ? { selector: wanted } : {}).then((answer) => {
    measured.value = asMeasured(answer) ?? measured.value;
    status.value = null;
  }, failed);
}

function choose(): void {
  panel.call({ tool: 'measure_text' }, { choose: true }).then(() => say(t('panel.choosing')), failed);
}

// The panel keeps the keyboard after its button was clicked: Esc here ends the pipette too.
function onKey(event: KeyboardEvent): void {
  if (event.key !== 'Escape' || !picking.value) return;
  event.preventDefault();
  stopPicking();
}
window.addEventListener('keydown', onKey);

const stopEvents = onPanelEvent((message) => {
  if (!message || typeof message !== 'object' || 'host' in message) return;
  const event = message as { type?: unknown; slot?: unknown; color?: { hex?: unknown }; managed?: unknown; result?: unknown };
  if (event.type === 'picked' && event.color && (event.slot === 'foreground' || event.slot === 'background')) {
    const hex = String(event.color.hex);
    setPicking(false);
    void setColor(event.slot, hex);
    // Taken from a window image in the display's colours (RB-080-77): the panel says so.
    say(t(event.managed === true ? 'panel.picked_managed' : 'panel.picked', { value: hex }), event.managed === true ? 'warn' : 'ok');
  } else if (event.type === 'picking') {
    setPicking(true, 'panel.picking');
  } else if (event.type === 'stopped') {
    setPicking(false, 'panel.stopped');
  } else if (event.type === 'measured') {
    measured.value = asMeasured(event.result) ?? measured.value;
  }
});
onBeforeUnmount(() => {
  stopEvents();
  window.removeEventListener('keydown', onKey);
});

panel.onSettings(() => {
  const wanted = panel.settings.value['color-picker.textMeasure.selector'];
  if (!selectorTouched && typeof wanted === 'string') selector.value = wanted;
});
void updateContrast();

const LEVELS = [
  { size: 'normal', label: 'panel.normal_text' },
  { size: 'large', label: 'panel.large_text' },
] as const;
const levelColumns = computed(() => [
  { id: 'size', header: t('panel.text_size'), meta: { class: { td: 'whitespace-normal' } } },
  { id: 'aa', header: 'AA' },
  { id: 'aaa', header: 'AAA' },
]);
/** Compact cells: the panel is often 400 px wide. */
const TABLE_UI = { th: 'px-2 py-1.5', td: 'px-2 py-1.5' };
const CARD_UI = { header: 'flex items-center gap-2 px-3 py-2 sm:px-3', body: 'flex flex-col gap-3 p-3 sm:p-3' };
</script>

<template>
  <PanelFrame :title="t('panels.picker')" icon="i-lucide-pipette" :status="status">
    <template #actions>
      <IconAction
        id="pick"
        :icon="picking ? 'i-lucide-circle-stop' : 'i-lucide-pipette'"
        :label="t(picking ? 'panel.pick_active' : 'panel.pick')"
        primary
        :aria-pressed="picking ? 'true' : 'false'"
        aria-describedby="pick-hint"
        :data-active="picking ? '' : undefined"
        @click="togglePick"
      />
    </template>

    <div class="flex flex-col gap-3">
      <p id="pick-hint" class="text-xs text-muted">{{ t('panel.pick_hint') }}</p>

      <UCard as="section" aria-labelledby="colors-title" :ui="CARD_UI">
        <template #header>
          <h2 id="colors-title" class="min-w-0 flex-1 truncate text-sm font-semibold text-highlighted">{{ t('panel.colors') }}</h2>
          <IconAction id="swap" icon="i-lucide-arrow-left-right" :label="t('panel.swap')" @click="swap" />
        </template>
        <div class="grid grid-cols-[repeat(auto-fit,minmax(220px,1fr))] gap-2">
          <ColorSlot
            v-for="slot in SLOTS"
            :key="slot"
            :which="slot"
            :value="texts[slot]"
            :formats="rating?.[slot] ?? null"
            :order="formats"
            @change="(value) => setColor(slot, value)"
            @copy="copy"
          />
        </div>
        <div class="flex flex-wrap items-baseline gap-2">
          <h3 class="text-sm font-semibold text-highlighted">{{ t('panel.contrast') }}</h3>
          <span id="ratio" class="text-2xl font-semibold tabular-nums text-highlighted">{{ rating ? t('panel.ratio', { ratio: Number(rating.ratio).toFixed(2) }) : '' }}</span>
        </div>
        <UTable id="levels" :data="[...LEVELS]" :columns="levelColumns" :ui="TABLE_UI" class="text-sm">
          <template #size-cell="{ row }">{{ t(row.original.label) }}</template>
          <template v-for="level in ['aa', 'aaa'] as const" :key="level" #[`${level}-cell`]="{ row }">
            <UBadge
              v-if="rating"
              :data-size="row.original.size"
              :data-level="level"
              :data-tone="rating[row.original.size]?.[level] ? 'ok' : 'error'"
              :color="rating[row.original.size]?.[level] ? 'success' : 'error'"
              :icon="rating[row.original.size]?.[level] ? 'i-lucide-circle-check' : 'i-lucide-circle-x'"
              variant="subtle"
              size="sm"
              :label="t(rating[row.original.size]?.[level] ? 'panel.pass' : 'panel.fail')"
            />
          </template>
        </UTable>
      </UCard>

      <PaletteCard
        :groups="palette"
        @read="readPalette"
        @clear="clearMarks('palette')"
        @mark="mark"
        @use="(slot, hex) => setColor(slot, hex)"
      />

      <MeasureCard
        v-model:selector="selector"
        :measured="measured"
        :failure="failure"
        @typed="selectorTouched = true"
        @run="measure"
        @choose="choose"
        @clear="clearMarks('measure_text')"
      />
    </div>
  </PanelFrame>
</template>
