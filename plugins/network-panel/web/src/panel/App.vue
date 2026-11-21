<script setup lang="ts">
// The network panel (RB-070-02, RB-080-37) on Nuxt UI (RB-080-84). It asks `list` every POLL_MS
// while visible, marks the requests only some views loaded (`partial`) or saw differently
// (`differs`), shows a request's views and body, and drives the session's switches; the tabs
// Breakpoints and Compose are the HTTP debugger, Sessions the saved captures, Mocks the rules,
// Endpoints the requests grouped by endpoint (RB-096-06). Clearing the site data asks first,
// inline (the frame shows no dialogs). The commands api_open and api_save post `apiOpen` and
// `apiSave`: the selected request into the API client, the shown ones saved (RB-096-08).
import { computed, nextTick, onBeforeUnmount, provide, ref, watch } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { onPanelEvent } from '../../../../web-common/panel-ui/src/events';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import PanelFrame from '../../../../web-common/panel-ui/src/components/PanelFrame.vue';
import PanelTabs from '../../../../web-common/panel-ui/src/components/PanelTabs.vue';
import BreakpointsTab from './BreakpointsTab.vue';
import ComposeTab from './ComposeTab.vue';
import EndpointsTab from './EndpointsTab.vue';
import MocksTab from './MocksTab.vue';
import RequestsTab from './RequestsTab.vue';
import SessionsTab from './SessionsTab.vue';
import { NETWORK_KEY, TABS, createNetwork, type Tab } from './store';

const POLL_MS = 2000;
const panel = usePanel();
const t = panel.t;
const net = createNetwork(panel);
provide(NETWORK_KEY, net);

const confirmNo = ref<{ $el: HTMLElement } | null>(null);

const tabs = computed(() =>
  (
    [
      ['requests', 'i-lucide-list'],
      ['endpoints', 'i-lucide-route'],
      ['breakpoints', 'i-lucide-octagon-pause'],
      ['compose', 'i-lucide-send'],
      ['sessions', 'i-lucide-history'],
      ['mocks', 'i-lucide-flask-conical'],
    ] as const
  ).map(([value, icon]) => ({ label: t('tabs.' + value), value, slot: value, icon })),
);

function askClear(): void {
  net.confirming.value = true;
  void nextTick(() => confirmNo.value?.$el.focus());
}

watch(net.tab, (now: Tab) => {
  if (now === 'breakpoints' || now === 'compose') void net.debug.refresh();
  if (now === 'sessions') void net.sessions.refresh();
  if (now === 'endpoints') void net.endpoints.refresh();
});

const stopEvents = onPanelEvent((raw) => {
  if (!raw || typeof raw !== 'object') return;
  const message = raw as Record<string, unknown>;
  if ('confirmClear' in message) askClear();
  if ('mocksChanged' in message) void net.mocks.refresh();
  if ('breakpointsChanged' in message || 'breakpointsEnabled' in message) void net.debug.refresh();
  if ('sessionsChanged' in message) void net.sessions.refresh();
  if (TABS.includes(message.tab as Tab)) net.tab.value = message.tab as Tab;
  // The commands api_open and api_save (RB-096-08).
  if (message.apiOpen === true) void net.api.open();
  if (message.apiSave === true) void net.api.saveShown();
  void net.refresh();
});

panel.onSettings(() => {
  document.title = t('panels.network');
  void net.refresh();
  void net.mocks.refresh();
});
const timer = setInterval(() => {
  if (document.hidden) return;
  void net.poll();
  if (net.tab.value === 'breakpoints' || net.tab.value === 'compose') void net.debug.poll();
  if (net.tab.value === 'endpoints') void net.endpoints.poll();
}, POLL_MS);
onBeforeUnmount(() => {
  clearInterval(timer);
  stopEvents();
});
</script>

<template>
  <PanelFrame :title="t('panels.network')" icon="i-lucide-network" :status="net.status.value">
    <template #actions>
      <IconAction id="export" icon="i-lucide-download" :label="t('toolbar.export')" @click="net.act({ command: 'har_export' }, {}, 'done.exported')" />
      <IconAction id="clear-site" icon="i-lucide-eraser" :label="t('toolbar.clearSite')" @click="askClear" />
    </template>

    <div class="flex h-full min-h-0 flex-col gap-2">
      <UAlert
        v-if="net.confirming.value"
        id="confirm"
        role="alertdialog"
        aria-labelledby="confirm-text"
        color="error"
        variant="subtle"
        icon="i-lucide-triangle-alert"
        :ui="{ root: 'px-3 py-2', title: 'text-sm font-normal', actions: 'gap-1.5' }"
        @keydown.esc="net.confirming.value = false"
      >
        <template #title>
          <span id="confirm-text">{{ t('confirm.text') }}</span>
        </template>
        <template #actions>
          <UButton id="confirm-yes" color="error" size="sm" :label="t('confirm.yes')" @click="net.clearSite()" />
          <UButton id="confirm-no" ref="confirmNo" color="neutral" variant="outline" size="sm" :label="t('confirm.no')" @click="net.confirming.value = false" />
        </template>
      </UAlert>

      <PanelTabs
        id="tabs"
        v-model="net.tab.value"
        :items="tabs"
        :unmount-on-hide="false"
        variant="link"
        size="sm"
        class="min-h-0 flex-1"
        :ui="{ root: 'min-h-0 min-w-0 gap-2', content: 'flex min-h-0 min-w-0 flex-1 flex-col' }"
      >
        <template #requests>
          <RequestsTab />
        </template>
        <template #endpoints>
          <EndpointsTab />
        </template>
        <template #breakpoints>
          <BreakpointsTab />
        </template>
        <template #compose>
          <ComposeTab />
        </template>
        <template #sessions>
          <SessionsTab />
        </template>
        <template #mocks>
          <MocksTab />
        </template>
      </PanelTabs>
    </div>
  </PanelFrame>
</template>
