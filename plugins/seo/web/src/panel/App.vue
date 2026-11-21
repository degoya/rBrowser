<script setup lang="ts">
// The SEO panel (manifest [[panels]] `seo`, RB-080-11; on Nuxt UI since RB-080-85): the link
// previews of the chosen platforms for the active view's page, from the tool `social_preview` -
// a card per platform with the variant its rule picks and why (RB-080-99). It asks once its
// texts arrived, on Refresh, and when the command `social_preview` (K G) posts the page it
// previewed (`{ type: 'event', message: { type: 'preview', url } }`). The share images come as
// `data:` URIs in the answer: the frame has no network. The answer may be as large as the panels
// window's message limit (RB-090-44): the panel asks for that `budget`. Above the cards the
// page's search result for desktop and mobile (RB-090-95), as the setting `seo.serpPreview` says.
import { computed, onBeforeUnmount, ref } from 'vue';
import { onPanelEvent } from '../../../../web-common/panel-ui/src/events';
import { answerBudget } from '../../../../web-common/panel-ui/src/limit';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import type { StatusLine } from '../../../../web-common/panel-ui/src/status';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import PanelFrame from '../../../../web-common/panel-ui/src/components/PanelFrame.vue';
import PlatformCard from './PlatformCard.vue';
import SerpPreview from './SerpPreview.vue';
import { isPreview, type Preview } from './model';
import { devicesShown, isSerp } from './serp';

const panel = usePanel();
const t = panel.t;

const preview = ref<Preview | null>(null);
const loading = ref(false);
const failure = ref<unknown>(null);
const serp = computed(() => {
  const value = preview.value?.serp;
  return isSerp(value) ? value : null;
});
const devices = computed(() => devicesShown(panel.settings.value['seo.serpPreview']));

const line = computed<StatusLine | null>(() => {
  void panel.strings.value;
  if (failure.value !== null) return { tone: 'error', text: t('panel.error', { reason: panel.error(failure.value) }) };
  if (panel.waiting.value) return { tone: 'busy', text: panel.strings.value['plugins.panel.waiting'] ?? '' };
  if (loading.value) return { tone: 'busy', text: t('panel.loading') };
  const status = preview.value?.status;
  if (preview.value && status !== 200) return { tone: 'warn', text: t('panel.status', { status: status ?? '-' }) };
  return null;
});

/** Only the latest request's answer shows and ends the loading (Refresh, then the command's
 * page: the first answer is stale). */
let latest = 0;

function load(url?: string): void {
  const request = ++latest;
  loading.value = true;
  failure.value = null;
  panel
    .call({ tool: 'social_preview' }, { ...(url ? { url } : {}), budget: answerBudget(panel.settings.value) })
    .then(
      (value) => {
        if (request !== latest) return;
        if (isPreview(value)) preview.value = value;
        else failure.value = new Error('plugins.error.bad_output');
      },
      (error: unknown) => {
        if (request === latest) failure.value = error;
      },
    )
    .finally(() => {
      if (request === latest) loading.value = false;
    });
}

// The command `social_preview` posts the page it previewed.
const stopEvents = onPanelEvent((message) => {
  const posted = message as { type?: unknown; url?: unknown } | null;
  if (posted?.type === 'preview') load(typeof posted.url === 'string' ? posted.url : undefined);
});
onBeforeUnmount(stopEvents);

panel.onSettings((first) => {
  if (first) load();
});
</script>

<template>
  <PanelFrame :title="t('panel.title')" icon="i-lucide-share-2" :status="line">
    <template #actions>
      <IconAction id="refresh" icon="i-lucide-refresh-cw" :label="t('panel.refresh')" :disabled="loading" @click="load()" />
    </template>

    <div class="flex flex-col gap-3">
      <p v-if="preview" id="page" class="flex flex-wrap items-center gap-1.5 text-sm text-muted">
        <UIcon name="i-lucide-globe" class="size-4 shrink-0" />
        <span class="min-w-0 break-all">{{ t('panel.page', { url: preview.finalUrl }) }}</span>
        <UBadge data-count color="neutral" variant="subtle" :label="t('panel.count', { count: preview.cards.length })" />
      </p>

      <SerpPreview v-if="serp && devices.length > 0" :serp="serp" :devices="devices" />

      <UEmpty
        v-if="!loading && preview && preview.cards.length === 0"
        id="empty"
        icon="i-lucide-share-2"
        :title="t('panel.empty')"
        variant="naked"
      />

      <div id="cards" class="grid grid-cols-[repeat(auto-fill,minmax(260px,1fr))] gap-3">
        <PlatformCard v-for="card in preview?.cards ?? []" :key="card.id" :preview="preview as Preview" :card="card" />
      </div>
    </div>
  </PanelFrame>
</template>
