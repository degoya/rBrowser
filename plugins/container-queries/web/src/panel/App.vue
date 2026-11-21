<script setup lang="ts">
// The container-queries panel (RB-094-01, docs/design.md 6): the overlay as a switch, every
// @container condition of the views with its bounds, then each container with its size per view,
// the next bounds, the matching conditions, the breakpoints that switch it and the reset of a
// width rBrowser set. It analyzes when it opens, on Analyze, and when a page script or the
// component says a width or the overlay changed. Loading, empty and failed are three states.
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { onPanelEvent } from '../../../../web-common/panel-ui/src/events';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import type { StatusLine } from '../../../../web-common/panel-ui/src/status';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import PanelFrame from '../../../../web-common/panel-ui/src/components/PanelFrame.vue';
import ContainerCard from './ContainerCard.vue';
import { type Analysis, type Condition, boundText, conditionText, readAnalysis, readEvent } from './model';

const panel = usePanel();
const t = panel.t;

const analysis = ref<Analysis | null>(null);
const failure = ref<string | null>(null);
const busy = ref(false);
const overlay = ref(false);
let again = false;
let timer: ReturnType<typeof setTimeout> | null = null;

async function analyze(): Promise<void> {
  if (busy.value) {
    again = true;
    return;
  }
  busy.value = true;
  failure.value = null;
  try {
    const answer = readAnalysis(await panel.call({ tool: 'analyze' }, {}));
    if (!answer) throw new Error('plugins.error.bad_output');
    analysis.value = answer;
    overlay.value = answer.overlay;
  } catch (error) {
    failure.value = panel.error(error);
  } finally {
    busy.value = false;
  }
  if (again) {
    again = false;
    void analyze();
  }
}

/** One analysis for a burst of events (a drag ends, a reset in several views). */
function soon(): void {
  if (timer !== null) clearTimeout(timer);
  timer = setTimeout(() => {
    timer = null;
    void analyze();
  }, 250);
}

async function run(tool: string, args: Record<string, unknown>): Promise<void> {
  failure.value = null;
  try {
    await panel.call({ tool }, args);
  } catch (error) {
    failure.value = panel.error(error);
  }
}

async function setOverlay(on: boolean): Promise<void> {
  overlay.value = on;
  await run('overlay', { on });
}

const resetAll = (): Promise<void> => run('resize', { reset: true });
const resetOne = (path: string, view: number): Promise<void> => run('resize', { reset: true, path, target: view });
const resizeOne = (path: string, view: number, width: number): Promise<void> => run('resize', { path, width, target: view });

const stop = onPanelEvent((message) => {
  const event = readEvent(message);
  if (event === null) return;
  if ('overlay' in event) overlay.value = event.overlay;
  else soon();
});
onMounted(() => {
  panel.onSettings((first) => {
    if (first) void analyze();
  });
});
onBeforeUnmount(() => {
  stop();
  if (timer !== null) clearTimeout(timer);
});

const empty = computed(() => analysis.value !== null && analysis.value.conditions.length === 0 && analysis.value.containers.length === 0);
const resized = computed(() => analysis.value?.containers.some((container) => container.views.some((view) => view.resized)) ?? false);
const unreadable = computed(() => analysis.value?.views.reduce((sum, view) => sum + view.unreadableSheets, 0) ?? 0);
/** The sentence of a view's failure code. */
const reason = (code: string | undefined): string => panel.error(new Error(code ?? 'plugins.error.host_failed'));
const failedViews = computed(() => analysis.value?.views.filter((view) => view.error) ?? []);
const conditionColumns = computed(() => [
  { id: 'condition', header: t('panel.columns.condition') },
  { id: 'bounds', header: t('panel.columns.bounds'), meta: { class: { td: 'tabular-nums' } } },
]);

const line = computed<StatusLine | null>(() => {
  if (failure.value) return { tone: 'error', text: failure.value };
  if (busy.value) return { tone: 'busy', text: t('panel.analyzing') };
  const current = analysis.value;
  if (!current) return null;
  const views = current.views.filter((view) => !view.error).length;
  return { tone: 'info', text: t('panel.summary', { conditions: current.conditions.length, containers: current.containers.length, views }) };
});
</script>

<template>
  <PanelFrame :title="t('panels.main')" icon="i-lucide-ruler" :status="line">
    <template #actions>
      <IconAction id="reset-all" icon="i-lucide-undo-2" :label="t('panel.reset_all')" :disabled="busy || !resized" @click="resetAll" />
      <IconAction id="analyze" icon="i-lucide-refresh-cw" :label="t('panel.analyze')" primary :disabled="busy" @click="analyze" />
    </template>

    <div class="flex flex-col gap-3">
      <USwitch id="overlay" :model-value="overlay" :label="t('panel.overlay')" :description="t('panel.overlay_hint')" @update:model-value="setOverlay" />

      <UAlert v-if="failure && !analysis" id="failed" role="alert" color="error" variant="subtle" icon="i-lucide-circle-x" :title="failure">
        <template #actions>
          <UButton id="retry" size="xs" color="neutral" variant="outline" :label="t('panel.retry')" @click="analyze" />
        </template>
      </UAlert>

      <template v-if="analysis">
        <p v-for="view in failedViews" :key="view.view" data-view-error class="text-xs text-error">
          {{ t('panel.view_failed', { view: view.label || `#${view.view}`, reason: reason(view.error) }) }}
        </p>
        <p v-if="unreadable > 0" id="unreadable" class="text-xs text-muted">{{ t('panel.unreadable', { n: unreadable }) }}</p>

        <UEmpty v-if="empty" id="empty" icon="i-lucide-ruler" :title="t('panel.empty')" variant="naked" />

        <section v-if="analysis.conditions.length > 0" aria-labelledby="conditions-title" class="flex flex-col gap-1">
          <h2 id="conditions-title" class="text-xs font-semibold uppercase text-muted">{{ t('panel.conditions', { n: analysis.conditions.length }) }}</h2>
          <UTable id="conditions" :data="analysis.conditions" :columns="conditionColumns" class="text-sm">
            <template #condition-cell="{ row }">
              <span class="flex flex-wrap items-center gap-1.5">
                <code data-condition>{{ conditionText(row.original as Condition) }}</code>
                <UBadge v-if="!(row.original as Condition).supported" data-unsupported color="warning" variant="subtle" size="sm" :label="t('panel.unsupported')" />
              </span>
            </template>
            <template #bounds-cell="{ row }">{{ (row.original as Condition).bounds.map(boundText).join(', ') }}</template>
          </UTable>
        </section>

        <section v-if="analysis.containers.length > 0" aria-labelledby="containers-title" class="flex flex-col gap-2">
          <h2 id="containers-title" class="text-xs font-semibold uppercase text-muted">{{ t('panel.containers', { n: analysis.containers.length }) }}</h2>
          <ContainerCard
            v-for="container in analysis.containers"
            :key="container.path"
            :container="container"
            :analysis="analysis"
            :busy="busy"
            @reset="(view) => resetOne(container.path, view)"
            @resize="(view, width) => resizeOne(container.path, view, width)"
          />
        </section>
      </template>
    </div>
  </PanelFrame>
</template>
