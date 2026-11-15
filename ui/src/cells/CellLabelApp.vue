<script setup lang="ts">
// The page `cell-label.html?view=<id>`: one per grid cell, in a CEF view the size of the cell's
// `label` rect (RB-010-04), so the label never covers a browser view.
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import UApp from '@nuxt/ui/components/App.vue';
import { useShell } from '../state/shell';
import CellLabel from './CellLabel.vue';
import { badgeOf, sessionsOf } from '../sessions/sessions';
import DesyncBadge from '../sync/DesyncBadge.vue';
import ViewAddressBar from '../sync/ViewAddressBar.vue';
import ExternalBadge from '../engines/ExternalBadge.vue';
import VitalsBadges from '../vitals/VitalsBadges.vue';
import type { VitalMetric } from '../ipc/protocol';

const props = defineProps<{ viewId: number }>();
const shell = useShell();
const { t } = useI18n();

const view = computed(() => shell.state.value?.views.find((v) => v.id === props.viewId) ?? null);
const active = computed(() => shell.state.value?.activeView === props.viewId);
const sessionColor = computed(() =>
  view.value === null ? null : (badgeOf(view.value, sessionsOf(shell.state.value))?.color ?? null),
);
const desync = computed(() => shell.state.value?.sync.desynced?.find((d) => d.view === props.viewId) ?? null);
const excluded = computed(() => shell.state.value?.sync.excluded?.includes(props.viewId) ?? false);
const decoupled = computed(
  () => shell.state.value?.sync.navigationDecoupled?.includes(props.viewId) ?? false,
);
const external = computed(() => shell.state.value?.externals?.find((e) => e.view === props.viewId) ?? null);
// The view's Web Vitals and resources (RB-060-10); a resource warning colours the whole label.
const metrics = computed(() => shell.state.value?.metrics?.find((m) => m.view === props.viewId) ?? null);
const resourceWarn = computed(
  () => metrics.value?.resources?.cpuWarn === true || metrics.value?.resources?.ramWarn === true,
);

// Until the size quick pick of RB-010-16 lands, a click on the size makes the view active too.
function activate(id: number): void {
  void shell.send({ type: 'setActiveView', view: id });
}

// The address field of a decoupled view (RB-020-06): only this view loads the address.
function navigate(url: string): void {
  void shell.send({ type: 'runCommand', id: 'view.navigate', args: { url, view: props.viewId } });
}

// The badge's click (RB-020-02): the view loads the active view's page.
function resync(id: number): void {
  void shell.send({ type: 'runCommand', id: 'sync.resync', args: { view: id } });
}

// The external view's buttons (RB-040-01): restart its engine's window, or bring it to the front.
function restartEngine(id: number): void {
  void shell.send({ type: 'runCommand', id: 'engine.restart', args: { view: id } });
}

// A vital's badge (RB-060-10): the core marks the element behind it in the view.
function highlightVital(view: number, metric: VitalMetric): void {
  void shell.send({ type: 'highlightVital', view, metric });
}

function showWindow(): void {
  void shell.send({ type: 'runCommand', id: 'view.external.raise' });
}
</script>

<template>
  <UApp>
    <main class="h-screen" :class="resourceWarn ? 'bg-error/10' : 'bg-default'">
      <CellLabel
        v-if="view !== null"
        :view="view"
        :active="active"
        :session-color="sessionColor"
        @activate="activate"
        @pick-size="activate"
      >
        <template v-if="decoupled" #address>
          <ViewAddressBar :url="view.url" :label="view.label" @navigate="navigate" />
        </template>
        <template #badges>
          <DesyncBadge :desync="desync" :excluded="excluded" @resync="resync" />
          <ExternalBadge :external="external" @restart="restartEngine" @show="showWindow" />
          <VitalsBadges :metrics="metrics" @highlight="highlightVital" />
        </template>
      </CellLabel>
      <p v-else class="px-1 text-xs text-muted">{{ t('shell.cell.missing', { view: viewId }) }}</p>
    </main>
  </UApp>
</template>
