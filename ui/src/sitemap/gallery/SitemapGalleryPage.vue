<script setup lang="ts">
// The sitemap gallery page (RB-090-86), the route `#/sitemap-gallery?run=<id>` of the shell's
// pages, which the app's gallery window shows (`cef_app/gallery_window.rs`): the run's facts,
// filters by breakpoint and by status, the contact sheet (`GallerySheet.vue`), the large view
// (`ShotViewer.vue`) and the list of errors. The run comes from the core (`getSitemapGallery`);
// the images from the window's own route `rbrowser://sitemap/`, which serves only this run's
// shots to this window. The plugin's `index.html` stays the portable export for the system
// browser.
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import UAlert from '@nuxt/ui/components/Alert.vue';
import UBadge from '@nuxt/ui/components/Badge.vue';
import URadioGroup from '@nuxt/ui/components/RadioGroup.vue';
import type { IpcClient } from '../../ipc/client';
import { IpcErrorCode, type SitemapGalleryPage } from '../../ipc/protocol';
import { IpcError } from '../../ipc/transport';
import ExternalLink from '../../links/ExternalLink.vue';
import GallerySheet from './GallerySheet.vue';
import { ALL, labelOf, problemsOf, runOfHash, shotOrder, type Problem, type ShotRef, type StatusFilter } from './gallery';
import ShotViewer from './ShotViewer.vue';

defineOptions({ inheritAttrs: false });
const props = withDefaults(
  defineProps<{
    client: IpcClient;
    /** The route; the window's own when absent. */
    hash?: string;
    /** How often a running capture is read again, in milliseconds. */
    refreshMs?: number;
  }>(),
  { hash: undefined, refreshMs: 5000 },
);
const { t } = useI18n();

const STATE_KEYS: Record<string, string> = {
  running: 'sitemap.galleryPage.state.running',
  cancelled: 'sitemap.galleryPage.state.cancelled',
  finished: 'sitemap.galleryPage.state.finished',
};

const run = ref<SitemapGalleryPage | null>(null);
const failure = ref<string | null>(null);
const breakpoint = ref<string>(ALL);
const status = ref<StatusFilter>('all');
const viewing = ref<ShotRef | null>(null);

const filter = computed(() => ({ breakpoint: breakpoint.value, status: status.value }));
const order = computed(() => (run.value === null ? [] : shotOrder(run.value, filter.value)));
const breakpointItems = computed(() => [
  { value: ALL, label: t('sitemap.galleryPage.filter.all') },
  ...(run.value?.breakpoints ?? []).map((bp) => ({ value: bp.name, label: labelOf(bp) })),
]);
const statusItems = computed(() => [
  { value: 'all', label: t('sitemap.galleryPage.filter.all') },
  { value: 'ok', label: t('sitemap.galleryPage.status.ok') },
  { value: 'problems', label: t('sitemap.galleryPage.status.problems') },
]);
const errors = computed(() =>
  (run.value?.pages ?? []).flatMap((page) => problemsOf(page).map((problem) => ({ url: page.url, problem }))),
);

function kindText(problem: Problem): string {
  if (problem.kind === 'http') return t('sitemap.galleryPage.kind.http', { status: problem.detail });
  if (problem.kind === 'redirect') return t('sitemap.galleryPage.kind.redirect', { url: problem.detail });
  return t('sitemap.galleryPage.kind.failed', { detail: problem.detail });
}

// A filter that hides the shot in the large view closes it.
watch(order, (shots) => {
  const shown = viewing.value;
  if (shown !== null && !shots.some((shot) => shot.page === shown.page && shot.breakpoint === shown.breakpoint)) {
    viewing.value = null;
  }
});

/** The newest read: an older answer that comes later is dropped. */
let reading = 0;
/** The route read last: the same one again is no new read. */
let readHash: string | null = null;

/** The next read of a running capture. */
let refreshTimer: ReturnType<typeof setTimeout> | null = null;

function stopRefresh(): void {
  if (refreshTimer !== null) clearTimeout(refreshTimer);
  refreshTimer = null;
}

/**
 * Reads the run `runId` names (the latest without); `quiet`: a running capture's next read,
 * which keeps what the page shows until the answer comes. While the run is running, it is read
 * again every `refreshMs`.
 */
async function read(runId: string | null, quiet: boolean): Promise<void> {
  stopRefresh();
  const mine = ++reading;
  if (!quiet) {
    run.value = null;
    failure.value = null;
    viewing.value = null;
  }
  try {
    const { type: _type, ...answer } = await props.client.expect(
      runId === null ? { type: 'getSitemapGallery' } : { type: 'getSitemapGallery', runId },
      'sitemapGallery',
    );
    if (mine !== reading) return;
    run.value = answer;
    if (answer.state === 'running') {
      refreshTimer = setTimeout(() => void read(answer.runId, true), props.refreshMs);
    }
  } catch (error) {
    if (mine !== reading || quiet) return;
    failure.value =
      error instanceof IpcError && error.code === IpcErrorCode.NotFound
        ? t('sitemap.galleryPage.error.not_found')
        : t('sitemap.galleryPage.error.failed');
  }
}

/** Reads the run the route names; again when the window opens a gallery (a new hash, also for
 * the same run: the core gives every opening an address of its own). */
function load(hash: string): void {
  if (hash === readHash) return;
  readHash = hash;
  void read(runOfHash(hash), false);
}

const onHash = (): void => load(window.location.hash);
onMounted(() => {
  load(props.hash ?? window.location.hash);
  if (props.hash === undefined) window.addEventListener('hashchange', onHash);
});
onBeforeUnmount(() => {
  window.removeEventListener('hashchange', onHash);
  stopRefresh();
  reading += 1;
});
</script>

<template>
  <main class="flex h-screen flex-col gap-4 overflow-y-auto bg-default px-4 py-4 text-default" data-testid="sitemap-gallery">
    <UAlert v-if="failure" color="error" variant="subtle" :title="failure" data-testid="gallery-failure" />
    <p v-else-if="run === null" class="text-sm text-muted" data-testid="gallery-loading">{{ t('sitemap.galleryPage.loading') }}</p>
    <template v-else>
      <header class="flex flex-col gap-1">
        <h1 class="text-lg font-semibold text-highlighted">{{ t('sitemap.galleryPage.title') }}</h1>
        <p class="flex flex-wrap items-center gap-2 text-sm text-muted">
          <ExternalLink :href="run.source" class="text-primary">{{ run.source }}</ExternalLink>
          <span>{{ run.runId }}</span>
          <UBadge :color="run.state === 'finished' ? 'success' : 'neutral'" variant="subtle" size="sm" data-testid="gallery-state">
            {{ t(STATE_KEYS[run.state] ?? 'sitemap.galleryPage.state.running') }}
          </UBadge>
          <span>{{ t('sitemap.galleryPage.summary', { pages: run.pages.length, breakpoints: run.breakpoints.length }) }}</span>
        </p>
      </header>
      <div class="flex flex-wrap gap-x-8 gap-y-2">
        <URadioGroup
          v-model="breakpoint"
          :items="breakpointItems"
          :legend="t('sitemap.galleryPage.filter.breakpoints')"
          orientation="horizontal"
          size="sm"
          data-testid="filter-breakpoint"
        />
        <URadioGroup
          v-model="status"
          :items="statusItems"
          :legend="t('sitemap.galleryPage.filter.status')"
          orientation="horizontal"
          size="sm"
          data-testid="filter-status"
        />
      </div>
      <UAlert
        v-if="run.totalPages !== undefined"
        color="info"
        variant="subtle"
        :title="t('sitemap.galleryPage.truncated', { shown: run.pages.length, total: run.totalPages })"
        data-testid="gallery-truncated"
      />
      <GallerySheet :run="run" :filter="filter" @open="viewing = $event" />
      <section class="flex flex-col gap-2" data-testid="gallery-errors">
        <h2 class="text-base font-semibold text-highlighted">{{ t('sitemap.galleryPage.errors.title') }}</h2>
        <p v-if="errors.length === 0" class="text-sm text-muted">{{ t('sitemap.galleryPage.errors.none') }}</p>
        <ul v-else class="flex flex-col gap-1 text-sm">
          <li v-for="(entry, at) in errors" :key="at" class="flex flex-wrap items-center gap-2" data-testid="gallery-error">
            <UBadge color="error" variant="subtle" size="sm" data-testid="gallery-error-kind">{{ kindText(entry.problem) }}</UBadge>
            <ExternalLink :href="entry.url" class="break-all text-primary">{{ entry.url }}</ExternalLink>
          </li>
        </ul>
      </section>
      <ShotViewer :run="run" :order="order" :shot="viewing" @show="viewing = $event" />
    </template>
  </main>
</template>
