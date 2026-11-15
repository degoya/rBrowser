<script setup lang="ts">
// The PWA panel (manifest [[panels]], RB-080-08; on Nuxt UI since RB-080-85): the page's checks
// in five tabs - findings (a click jumps to the finding's place in its tab), the manifest as
// tables, every icon with its preview (the maskable safe zone drawn), the service worker and the
// offline test, Chromium's installability errors. The icons come as `data:` URIs from the tool
// `icon` (the frame has no network). "Check again" calls the tool `check`; "Send to the findings
// panel" the command `check`, whose `checked` event then starts no second check.
import { computed, nextTick, onBeforeUnmount, reactive, ref } from 'vue';
import { onPanelEvent } from '../../../../web-common/panel-ui/src/events';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import type { StatusLine } from '../../../../web-common/panel-ui/src/status';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import PanelFrame from '../../../../web-common/panel-ui/src/components/PanelFrame.vue';
import FindingsTable from './FindingsTable.vue';
import IconGrid from './IconGrid.vue';
import RowsTable from './RowsTable.vue';
import { anchorOf, type Answer, type Finding, findingsOf, type IconPreview, installLines, manifestRows, previewUrls, SECTIONS, type Section, sectionOf, text, workerRows } from './model';

/** What one answer may hold: the panels window refuses more than 64 KiB. */
const BUDGET = 60_000;
const SECTION_ICON: Record<Section, string> = {
  findings: 'i-lucide-triangle-alert',
  manifest: 'i-lucide-file-code',
  icons: 'i-lucide-image',
  sw: 'i-lucide-globe',
  install: 'i-lucide-download',
};

const panel = usePanel();
const strings = panel.strings;
const tx = (key: string, params?: Record<string, unknown>): string => text(strings.value, key, params);

const answer = ref<Answer | null>(null);
const pageIndex = ref(0);
const loading = ref(false);
const failure = ref<string | null>(null);
const reported = ref<number | null>(null);
const tab = ref<Section>('findings');
const previews = reactive(new Map<string, IconPreview>());
/** The panel's own report runs the command, whose answer posts `checked`: no second check. */
let reporting = false;

const page = computed(() => answer.value?.pages[pageIndex.value] ?? null);
const pageItems = computed(() => (answer.value?.pages ?? []).map((one, index) => ({ label: one.url, value: index })));
const notes = computed(() =>
  [...(answer.value?.skipped ?? []).map((skip) => tx('panel.skipped', skip)), answer.value?.cut ? tx('panel.cut') : ''].filter(Boolean),
);
const tabs = computed(() =>
  SECTIONS.map((name) => ({
    label: tx(`panel.sections.${name}`),
    value: name,
    slot: name,
    icon: SECTION_ICON[name],
    badge: name === 'findings' && answer.value && page.value ? String(findingsOf(answer.value, page.value).length) : undefined,
  })),
);

const line = computed<StatusLine | null>(() => {
  if (failure.value) return { tone: 'error', text: failure.value };
  if (panel.waiting.value) return { tone: 'busy', text: strings.value['plugins.panel.waiting'] ?? '' };
  if (loading.value) return { tone: 'busy', text: tx('panel.loading') };
  if (reported.value !== null) return { tone: 'ok', text: tx('panel.reported', { count: reported.value }) };
  return null;
});

async function loadPreviews(): Promise<void> {
  const current = page.value;
  if (!current) return;
  for (const url of previewUrls(current)) {
    if (previews.has(url)) continue;
    try {
      const icon = (await panel.call({ tool: 'icon' }, { url })) as { dataUri?: string | null; tooLarge?: boolean; bytes?: number };
      previews.set(url, { dataUri: icon.dataUri ?? null, tooLarge: icon.tooLarge === true, bytes: icon.bytes ?? 0 });
    } catch {
      previews.set(url, { dataUri: null, tooLarge: false, bytes: 0 });
    }
  }
}

function load(): void {
  loading.value = true;
  failure.value = null;
  reported.value = null;
  previews.clear();
  panel
    .call({ tool: 'check' }, { budget: BUDGET })
    .then(
      (value) => {
        answer.value = value as Answer;
        pageIndex.value = Math.min(pageIndex.value, Math.max(answer.value.pages.length - 1, 0));
        void loadPreviews();
      },
      (error: unknown) => {
        failure.value = panel.error(error);
      },
    )
    .finally(() => {
      loading.value = false;
    });
}

function report(): void {
  reporting = true;
  failure.value = null;
  panel
    .call({ command: 'check' }, {})
    .then(
      (value) => {
        reported.value = (value as { findings?: number } | null)?.findings ?? 0;
      },
      (error: unknown) => {
        failure.value = panel.error(error);
      },
    )
    .finally(() => {
      reporting = false;
    });
}

function choosePage(value: unknown): void {
  pageIndex.value = Number(value);
  void loadPreviews();
}

/** Focuses the element `id` once its tab shows it (the tab's content appears a frame later). */
function focusWhenShown(id: string, fallback: string, frames = 10): void {
  const target = document.getElementById(id) ?? document.getElementById(fallback);
  if (target && target.offsetParent !== null) {
    target.scrollIntoView?.({ block: 'start' });
    target.focus();
  } else if (frames > 0) {
    requestAnimationFrame(() => focusWhenShown(id, fallback, frames - 1));
  } else {
    target?.focus();
  }
}

/** A finding's place: its section's tab, then the element its anchor names. */
function jump(finding: Finding): void {
  tab.value = sectionOf(finding);
  const anchor = anchorOf(finding);
  void nextTick(() => focusWhenShown(anchor, tab.value));
}

// The command `check` posts `checked`; the panel's own report starts no second check.
const stopEvents = onPanelEvent((message) => {
  if ((message as { type?: unknown } | null)?.type === 'checked' && !reporting) load();
});
onBeforeUnmount(stopEvents);

panel.onSettings((first) => {
  if (first) load();
});
</script>

<template>
  <PanelFrame :title="tx('panels.pwa')" icon="i-lucide-app-window" :status="line">
    <template #actions>
      <IconAction id="report" icon="i-lucide-list-checks" :label="tx('panel.report')" :disabled="!answer" @click="report" />
      <IconAction id="refresh" icon="i-lucide-refresh-cw" :label="tx('panel.refresh')" primary :disabled="loading" @click="load" />
    </template>

    <div class="flex flex-col gap-3">
      <UAlert
        v-if="notes.length > 0"
        id="notes"
        color="info"
        variant="subtle"
        icon="i-lucide-info"
        :title="notes.join(' ')"
        :ui="{ root: 'px-3 py-1.5', title: 'text-sm font-normal' }"
      />

      <UEmpty v-if="!loading && !failure && !page" id="empty" icon="i-lucide-app-window" :title="tx('panel.empty')" variant="naked" />

      <template v-if="answer && page">
        <div class="flex flex-wrap items-center gap-2 text-sm text-muted">
          <span id="views" class="flex items-center gap-1.5">
            <UIcon name="i-lucide-monitor" class="size-4 shrink-0" />
            <span>{{ tx('panel.views', { views: page.views.join(', ') }) }}</span>
          </span>
          <USelect
            v-if="pageItems.length > 1"
            id="page"
            class="min-w-0 flex-1"
            size="sm"
            :items="pageItems"
            :model-value="pageIndex"
            :aria-label="tx('panel.page')"
            @update:model-value="choosePage"
          />
        </div>

        <UTabs id="tabs" v-model="tab" :items="tabs" :unmount-on-hide="false" variant="link" size="sm" :ui="{ list: 'overflow-x-auto', trigger: 'shrink-0' }">
          <template #findings>
            <section id="findings" tabindex="-1" class="pt-2 outline-none">
              <FindingsTable :answer="answer" :page="page" @jump="jump" />
            </section>
          </template>
          <template #manifest>
            <section id="manifest" tabindex="-1" class="flex flex-col gap-2 pt-2 outline-none">
              <p v-if="!page.manifest" class="text-sm text-muted">{{ tx('panel.manifest.none') }}</p>
              <template v-else>
                <RowsTable :rows="manifestRows(strings, page.manifest)" :head="[tx('panel.table.field'), tx('panel.manifest.value')]" />
                <RowsTable :rows="page.manifest.table" :head="[tx('panel.manifest.member'), tx('panel.manifest.value')]" anchors />
              </template>
            </section>
          </template>
          <template #icons>
            <section id="icons" tabindex="-1" class="pt-2 outline-none">
              <IconGrid :page="page" :previews="previews" />
            </section>
          </template>
          <template #sw>
            <section id="sw" tabindex="-1" class="pt-2 outline-none">
              <RowsTable :rows="workerRows(strings, page)" :head="[tx('panel.table.field'), tx('panel.manifest.value')]" />
            </section>
          </template>
          <template #install>
            <section id="install" tabindex="-1" class="pt-2 outline-none">
              <ul class="flex flex-col gap-1 text-sm">
                <li v-for="entry in installLines(strings, page)" :key="entry" class="flex items-start gap-1.5">
                  <UIcon
                    :name="page.installability.checked && page.installability.errors.length === 0 ? 'i-lucide-circle-check' : 'i-lucide-circle-alert'"
                    class="mt-0.5 size-4 shrink-0"
                    :class="page.installability.checked && page.installability.errors.length === 0 ? 'text-success' : 'text-warning'"
                  />
                  <span class="break-all">{{ entry }}</span>
                </li>
              </ul>
            </section>
          </template>
        </UTabs>
      </template>
    </div>
  </PanelFrame>
</template>
