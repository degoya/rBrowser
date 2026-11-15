<script setup lang="ts">
// The simulators picker (RB-070-08) on Nuxt UI (RB-080-86): Off and the simulations in their
// groups, a strength slider, "Apply to every view" (or Shift with the choice) and every view's
// simulation in words. A choice applies at once through the tool `set`; the strength is sent only
// once the slider was moved since the last choice. The switch acts at once in both directions
// (RB-080-75): on, every view gets the active view's simulation and strength; off, the active
// view keeps it and the other views showing the same simulation lose it.
import { computed, nextTick, onBeforeUnmount, ref } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import type { StatusLine } from '../../../../web-common/panel-ui/src/status';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import PanelFrame from '../../../../web-common/panel-ui/src/components/PanelFrame.vue';
import { onPanelEvent } from '../../../../web-common/panel-ui/src/events';
import { readState, request, type State, strengthFor, typeName, viewRow } from './model';
import SimulationPicker from './SimulationPicker.vue';

const panel = usePanel();
const t = panel.t;
/** Nothing is drawn before the texts arrived, so no key ever shows (RB-080-59). */
const ready = panel.ready;

const state = ref<State>({ active: null, views: [], catalog: [] });
const selection = ref('none');
/** The slider, in percent; off for a simulation without a strength. */
const strength = ref(100);
const strengthOff = ref(true);
const everyView = ref(false);
const line = ref<StatusLine | null>(null);
/** Whether the slider was moved since the last choice: only then is a strength sent. */
let moved = false;

const active = computed(() => state.value.views.find((view) => view.view === state.value.active));

function say(tone: StatusLine['tone'], text: string): void {
  if (panel.ready.value) line.value = { tone, text };
}
const failed = (error: unknown): void => say('error', panel.error(error));

/** The slider for `type`: its strength in the active view, else its default; off without one. */
function syncSlider(type: string): void {
  const own = strengthFor(state.value, type);
  strengthOff.value = own === null;
  strength.value = Math.round((own ?? 1) * 100);
}

function take(value: unknown): void {
  const read = readState(value);
  state.value = { active: read.active, views: read.views, catalog: read.catalog.length > 0 ? read.catalog : state.value.catalog };
  selection.value = active.value?.type ?? selection.value;
  syncSlider(selection.value);
}

function choose(all: boolean): void {
  const type = selection.value;
  const severity = moved && !strengthOff.value ? strength.value / 100 : null;
  const label = active.value?.label ?? '';
  panel.call({ tool: 'set' }, request(type, severity, all)).then((answer) => {
    take(answer);
    const name = typeName(t, type);
    say('ok', all ? t('panel.applied_all', { type: name }) : t('panel.applied', { type: name, label }));
  }, failed);
}

function picked(type: string, shift: boolean): void {
  moved = false;
  syncSlider(type);
  choose(shift || everyView.value);
}

// The slider commits a step of the keyboard before it takes the new value, so the choice is
// made once it has.
function committed(): void {
  void nextTick(() => {
    moved = true;
    choose(everyView.value);
  });
}

/** RB-080-75: the switch acts at once. On, every view gets the active view's simulation and
 * strength; off, the active view keeps it and the other views showing the same simulation lose it
 * (a view with its own, different one keeps that). */
function everyViewChanged(all: boolean): void {
  everyView.value = all;
  const own = active.value;
  if (!own) return;
  const name = typeName(t, own.type);
  if (all) {
    panel.call({ tool: 'set' }, request(own.type, own.severity, true)).then((answer) => {
      take(answer);
      say('ok', t('panel.applied_all', { type: name }));
    }, failed);
    return;
  }
  const others = own.type === 'none' ? [] : state.value.views.filter((view) => view.view !== own.view && view.type === own.type);
  Promise.all(others.map((view) => panel.call({ tool: 'set' }, { type: 'none', target: view.view }))).then((answers) => {
    const last = answers.at(-1);
    if (last !== undefined) take(last);
    say('ok', t('panel.kept_active', { type: name, label: own.label }));
  }, failed);
}

function refresh(): void {
  panel.call({ tool: 'set' }, {}).then(take, failed);
}

const stopEvents = onPanelEvent((message) => {
  if ((message as { type?: unknown } | null)?.type === 'state') take(message);
});
onBeforeUnmount(stopEvents);
// Nothing starts before the texts arrived (RB-080-59); then the views' simulations.
panel.onSettings((first) => {
  if (first) refresh();
});
// The active view may have changed while the picker was in the background.
const onFocus = (): void => {
  if (panel.ready.value) refresh();
};
window.addEventListener('focus', onFocus);
onBeforeUnmount(() => window.removeEventListener('focus', onFocus));

const CARD_UI = { header: 'px-3 py-2 sm:px-3', body: 'flex flex-col gap-3 p-3 sm:p-3' };
</script>

<template>
  <PanelFrame v-if="ready" :title="t('panel.title')" icon="i-lucide-glasses" :status="line">
    <template #actions>
      <IconAction id="refresh" icon="i-lucide-refresh-cw" :label="t('panel.refresh')" @click="refresh" />
    </template>

    <div class="flex flex-col gap-3">
      <p class="flex min-w-0 items-center gap-1.5 text-sm text-muted">
        <UIcon name="i-lucide-monitor-smartphone" class="size-4 shrink-0" />
        <span id="active" class="truncate" :title="active?.label">{{ active ? t('panel.view', { label: active.label }) : t('panel.no_view') }}</span>
      </p>

      <UCard :ui="CARD_UI">
        <SimulationPicker v-model="selection" :catalog="state.catalog" @pick="picked" @enter="(shift) => choose(shift || everyView)" />
      </UCard>

      <UCard :ui="CARD_UI">
        <UFormField size="sm" :label="t('panel.strength')" :hint="t('panel.strength_value', { value: strength })" :help="t('panel.strength_hint')">
          <USlider
            id="strength"
            v-model="strength"
            :min="0"
            :max="100"
            :step="5"
            :disabled="strengthOff"
            :aria-label="t('panel.strength')"
            :aria-valuetext="t('panel.strength_value', { value: strength })"
            @change="committed"
          />
        </UFormField>
        <USwitch id="all" size="sm" :model-value="everyView" :label="t('panel.all_views')" :description="t('panel.shift_hint')" @update:model-value="everyViewChanged" />
      </UCard>

      <UCard as="section" aria-labelledby="views-title" :ui="CARD_UI">
        <template #header>
          <h2 id="views-title" class="text-sm font-semibold text-highlighted">{{ t('panel.views') }}</h2>
        </template>
        <ul id="views" class="flex flex-col gap-1 text-sm">
          <li
            v-for="view in state.views"
            :key="view.view"
            :aria-current="view.view === state.active ? 'true' : undefined"
            class="border-s-2 ps-2 break-words"
            :class="view.view === state.active ? 'border-primary font-medium text-highlighted' : 'border-transparent'"
          >{{ viewRow(t, view) }}</li>
        </ul>
      </UCard>
    </div>
  </PanelFrame>
</template>
