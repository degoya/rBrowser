<script setup lang="ts">
// The overlays panel (RB-060-03) on Nuxt UI (RB-080-86): the overlays as switches grouped by kind
// (a switch acts on every view, Shift-click on the active view only), "all off" as a header
// action, the selector with its matches in every view and the state force-state forces, what
// each view reports, and the tab order compared across the views (RB-070-12). The switches show
// the component's state (`state`, then its `state` events); a switch just flipped shows its new
// position until the next state arrives.
import { computed, onBeforeUnmount, ref } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import type { CallTarget } from '../../../../web-common/panel-ui/src/panel';
import type { StatusLine } from '../../../../web-common/panel-ui/src/status';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import PanelFrame from '../../../../web-common/panel-ui/src/components/PanelFrame.vue';
import type { State } from '../types';
import { onPanelEvent } from '../../../../web-common/panel-ui/src/events';
import { FORCED_STATES, GROUPS, isOn, selectorOf, setArgs, type Toggle, Views } from './model';
import OverlaySwitches from './OverlaySwitches.vue';
import SelectorFields from './SelectorFields.vue';
import { asDifferences, type Differences, diffLines } from './tab-diffs';

const panel = usePanel();
const t = panel.t;

const state = ref<State | undefined>();
/** Switches flipped since the last state, by overlay. */
const flipped = ref<Partial<Record<Toggle, boolean>>>({});
const differences = ref<Differences | undefined>();
const selector = ref('');
const forced = ref<string>(FORCED_STATES[0]);
const line = ref<StatusLine | null>(null);
const views = new Views();
/** Counts what the views reported, so their lines are drawn again. */
const reported = ref(0);

const shown = (toggle: Toggle): boolean => flipped.value[toggle] ?? isOn(state.value, toggle);
const viewLines = computed(() => {
  void reported.value;
  return views.lines(t, (overlay) => isOn(state.value, overlay));
});
const tabLines = computed(() => (differences.value ? diffLines(t, differences.value) : []));

function takeState(value: unknown): void {
  if (!value || typeof value !== 'object') return;
  state.value = value as State;
  flipped.value = {};
  // The highlight's selector fills an empty field nobody is typing in.
  if (!selector.value && document.activeElement?.id !== 'selector') selector.value = selectorOf(state.value);
}

const failed = (error: unknown): void => {
  line.value = { tone: 'error', text: panel.error(error) };
};

function run(target: CallTarget, args: unknown): void {
  panel.call(target, args).then((answer) => {
    line.value = null;
    const tabOrder = asDifferences((answer as { differences?: unknown } | null)?.differences);
    if (tabOrder) differences.value = tabOrder;
    const matches = (answer as { matches?: { count?: unknown }[] } | null)?.matches;
    if (Array.isArray(matches)) {
      const total = matches.reduce((sum, entry) => sum + (typeof entry?.count === 'number' ? entry.count : 0), 0);
      line.value = { tone: 'info', text: t('panel.found', { count: total, views: matches.length }) };
    }
  }, failed);
}

function set(toggle: Toggle, enabled: boolean, shift: boolean): void {
  flipped.value = { ...flipped.value, [toggle]: enabled };
  run({ tool: 'set' }, setArgs(toggle, enabled, selector.value, forced.value, shift));
}

function find(): void {
  const wanted = selector.value.trim();
  if (wanted) run({ command: 'highlight' }, { selector: wanted });
  else failed('plugin.overlays.errors.no_selector');
}

const stopEvents = onPanelEvent((message, view) => {
  const posted = message as { type?: unknown; state?: unknown; differences?: unknown } | null;
  if (posted?.type === 'state') takeState(posted.state);
  else if (posted?.type === 'tab-order') differences.value = asDifferences(posted.differences) ?? differences.value;
  else if (views.take(view, message)) reported.value += 1;
});

onBeforeUnmount(stopEvents);

// The switches start from what every view shows, unless a state event came first.
panel.onSettings((first) => {
  if (!first) return;
  panel.call({ tool: 'state' }).then((answer) => {
    const overlays = (answer as { overlays?: State['all'] } | null)?.overlays;
    if (overlays && !state.value) takeState({ all: overlays, views: {} });
  }, failed);
});

const CARD_UI = { header: 'px-3 py-2 sm:px-3', body: 'flex flex-col gap-3 p-3 sm:p-3' };
</script>

<template>
  <PanelFrame :title="t('panels.main')" icon="i-lucide-layers" :status="line">
    <template #actions>
      <IconAction id="off" icon="i-lucide-eye-off" :label="t('panel.off')" @click="run({ command: 'off' }, {})" />
    </template>

    <div class="flex flex-col gap-3">
      <p id="shift-hint" class="flex items-center gap-1.5 text-xs text-muted">
        <UIcon name="i-lucide-info" class="size-4 shrink-0" />
        <span>{{ t('panel.shift_hint') }}</span>
      </p>

      <UCard v-for="group in GROUPS" :key="group.id" as="section" :data-group="group.id" :aria-labelledby="'group-' + group.id" :ui="CARD_UI">
        <template #header>
          <h2 :id="'group-' + group.id" class="text-sm font-semibold text-highlighted">{{ t('panel.group.' + group.id) }}</h2>
        </template>
        <SelectorFields v-if="group.id === 'selector'" v-model:selector="selector" v-model:forced="forced" @find="find" @step="(by) => run({ command: 'highlight' }, { step: by })" />
        <OverlaySwitches :toggles="group.toggles" :shown="shown" @set="set" />
        <div v-if="group.id === 'accessibility'" class="flex flex-col gap-2 border-t border-default pt-3">
          <div class="flex flex-wrap items-center gap-2">
            <h3 id="tab-title" class="flex-1 text-sm font-medium text-highlighted">{{ t('panel.tab_title') }}</h3>
            <UButton id="tab-compare" size="xs" color="neutral" variant="outline" icon="i-lucide-arrow-left-right" :label="t('panel.tab_compare')" @click="run({ tool: 'tab_order' }, {})" />
          </div>
          <ul id="tab-diffs" aria-live="polite" class="flex flex-col gap-1 text-sm">
            <li v-for="(text, index) in tabLines" :key="index" class="break-words">{{ text }}</li>
          </ul>
        </div>
      </UCard>

      <UCard as="section" aria-labelledby="views-title" :ui="CARD_UI">
        <template #header>
          <h2 id="views-title" class="text-sm font-semibold text-highlighted">{{ t('panel.views') }}</h2>
        </template>
        <ul v-if="viewLines.length > 0" id="views" class="divide-y divide-default text-sm">
          <li v-for="text in viewLines" :key="text" class="py-1.5 break-words first:pt-0 last:pb-0">{{ text }}</li>
        </ul>
        <p v-else id="empty" class="text-sm text-muted">{{ t('panel.empty') }}</p>
      </UCard>
    </div>
  </PanelFrame>
</template>
