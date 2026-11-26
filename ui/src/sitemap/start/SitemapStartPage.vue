<script setup lang="ts">
// The sitemap capture's start page (RB-099-09), the route `sitemap-start` of the pages window:
// `sitemap.capture` without arguments - the menu, the shortcut, the sitemap bar's Start - opens
// it. It finds the project's pages first (a progress bar, Cancel) and shows them as a tree by
// path segments with their counts (`SitemapTree.vue`); the person chooses pages and areas (All,
// None, a search over the URLs that All and None follow), the environment the pages move to and
// all or only the visible views (RB-099-08), and sees what that makes: "N pages x M views = K
// shots". Start sends `sitemap.capture` with the discovery's id and the chosen URLs - the core
// takes only pages of that discovery and closes this window once the run has started; Start
// waits for a choice. Close or Escape closes the window. Loading, nothing found and failed are
// three states, a failure says what to do. The last views and environment are remembered.
// A failure or a refusal is said at once (`role="alert"`); when the search fails or is cancelled
// its Cancel goes, so the focus moves to "Find the pages again", never to the body (RB-099-21).
import { computed, markRaw, nextTick, onScopeDispose, ref, shallowRef, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import UAlert from '@nuxt/ui/components/Alert.vue';
import UButton from '@nuxt/ui/components/Button.vue';
import UFormField from '@nuxt/ui/components/FormField.vue';
import UInput from '@nuxt/ui/components/Input.vue';
import UProgress from '@nuxt/ui/components/Progress.vue';
import URadioGroup from '@nuxt/ui/components/RadioGroup.vue';
import USelect from '@nuxt/ui/components/Select.vue';
import type { IpcClient } from '../../ipc/client';
import { useShell } from '../../state/shell';
import { START_ICONS } from './icons';
import SitemapTree from './SitemapTree.vue';
import { createStart, loadChoice, pageStorage, saveChoice } from './start';
import { failureDetail, failureKey } from './failure';
import { Selection, captureArgs, flatten, search, type ViewsChoice } from './tree';

defineOptions({ inheritAttrs: false });
const props = defineProps<{ client: IpcClient }>();
const { t } = useI18n();
const shell = useShell();
const start = createStart(props.client);

/** The environment choice "as found": the pages are not moved. */
const AS_FOUND = '-';
const remembered = loadChoice(pageStorage());
const views = ref<ViewsChoice>(remembered.views);
const env = ref<string>(remembered.env ?? AS_FOUND);
const query = ref('');
const version = ref(0);
const selection = shallowRef<Selection | null>(null);
const treeView = ref<InstanceType<typeof SitemapTree> | null>(null);
const cancelButton = ref<{ $el?: HTMLElement } | null>(null);
const againButton = ref<{ $el?: HTMLElement } | null>(null);

const state = computed(() => start.state.value);
const discovery = computed(() => ('discovery' in state.value ? state.value.discovery : null));
const tree = computed(() => (discovery.value === null ? null : flatten(discovery.value)));
const scope = computed(() => (tree.value === null ? null : search(tree.value.pages, query.value)));

watch(discovery, (now, before) => {
  if (now === null || now.discoveryId === before?.discoveryId) return;
  selection.value = markRaw(new Selection(now.pages.length));
  version.value += 1;
  void nextTick(() => treeView.value?.focus());
});

const environments = computed(() => shell.state.value?.environments ?? []);
const envItems = computed(() => [
  { label: t('sitemap.start.env_as_found'), value: AS_FOUND },
  ...environments.value.map((environment) => ({ label: environment.id, value: environment.id })),
]);
// A remembered environment the project no longer has falls back to "as found".
watch(environments, (now) => {
  if (env.value !== AS_FOUND && !now.some((environment) => environment.id === env.value)) env.value = AS_FOUND;
});

const viewCount = computed(() => discovery.value?.views[views.value] ?? 0);
const viewItems = computed(() => [
  { value: 'all', label: t('sitemap.start.views_all', { count: discovery.value?.views.all ?? 0 }) },
  {
    value: 'visible',
    label: t('sitemap.start.views_visible', { count: discovery.value?.views.visible ?? 0 }),
    disabled: (discovery.value?.views.visible ?? 0) === 0,
  },
]);

const chosen = computed(() => {
  void version.value;
  return selection.value?.count ?? 0;
});
const counter = computed(() =>
  t('sitemap.start.counter', { pages: chosen.value, views: viewCount.value, shots: chosen.value * viewCount.value }),
);
/** Why Start waits; `null` when it can start. */
const blocked = computed(() => {
  const found = discovery.value;
  if (found === null || state.value.phase === 'starting') return '';
  if (chosen.value === 0) return t('sitemap.start.choose_first');
  if (chosen.value > found.maxPages) return t('sitemap.start.too_many', { max: found.maxPages });
  if (viewCount.value === 0) return t('sitemap.start.no_views');
  return null;
});

function changed(): void {
  version.value += 1;
}

function chooseAll(on: boolean): void {
  selection.value?.setAll(on, scope.value);
  changed();
}

function begin(): void {
  const found = discovery.value;
  if (found === null || selection.value === null || blocked.value !== null) return;
  const environment = env.value === AS_FOUND ? null : env.value;
  saveChoice(pageStorage(), { views: views.value, env: environment });
  void start.start(captureArgs(found.discoveryId, selection.value.urls(found.pages), environment, views.value));
}

function onEscape(event: KeyboardEvent): void {
  // A choice's open list closes first.
  if (event.defaultPrevented) return;
  event.preventDefault();
  void start.close();
}

const failure = computed(() => (state.value.phase === 'failed' ? state.value.message : ''));

watch(
  () => state.value.phase,
  (phase) => {
    if (phase === 'failed' || phase === 'cancelled') void nextTick(() => againButton.value?.$el?.focus());
  },
);

// The core opens the page again with a new address: the pages are found again.
const rediscover = (): void => {
  if (window.location.hash.startsWith('#/sitemap-start')) void start.discover();
};
window.addEventListener('hashchange', rediscover);
onScopeDispose(() => window.removeEventListener('hashchange', rediscover));
void start.discover();
void nextTick(() => cancelButton.value?.$el?.focus());
</script>

<template>
  <main
    class="flex h-screen flex-col gap-3 bg-default p-4 text-sm text-default"
    :aria-label="t('sitemap.start.title')"
    data-testid="sitemap-start"
    @keydown.escape="onEscape"
  >
    <h1 class="text-base font-semibold text-highlighted">{{ t('sitemap.start.title') }}</h1>

    <section v-if="state.phase === 'finding'" class="flex flex-col gap-2" role="status" data-testid="sitemap-start-finding">
      <p>{{ t('sitemap.start.finding') }}</p>
      <UProgress animation="carousel" size="sm" />
      <div>
        <UButton ref="cancelButton" color="neutral" variant="outline" :label="t('sitemap.start.cancel')" data-testid="sitemap-start-cancel" @click="start.cancel()" />
      </div>
    </section>

    <section v-else-if="state.phase === 'cancelled'" class="flex flex-col items-start gap-2" data-testid="sitemap-start-cancelled">
      <p class="text-muted">{{ t('sitemap.start.cancelled') }}</p>
      <UButton ref="againButton" :label="t('sitemap.start.find_again')" data-testid="sitemap-start-again" @click="start.discover()" />
    </section>

    <section v-else-if="state.phase === 'failed'" class="flex flex-col items-start gap-2" data-testid="sitemap-start-failed">
      <UAlert color="error" variant="subtle" role="alert" :title="t(failureKey(failure))" :description="failureDetail(failure)" />
      <UButton ref="againButton" :label="t('sitemap.start.find_again')" data-testid="sitemap-start-again" @click="start.discover()" />
    </section>

    <template v-else-if="discovery !== null && tree !== null && selection !== null">
      <p class="truncate text-muted" :title="discovery.source" data-testid="sitemap-start-source">
        {{ t('sitemap.start.found', { count: discovery.pages.length, source: discovery.source }) }}
      </p>
      <UAlert
        v-if="discovery.truncated"
        color="warning"
        variant="subtle"
        role="status"
        :title="t('sitemap.start.truncated')"
        data-testid="sitemap-start-truncated"
      />
      <div class="flex flex-wrap items-center gap-2">
        <UInput
          v-model="query"
          class="min-w-48 flex-1"
          :icon="START_ICONS.startSearch"
          :placeholder="t('sitemap.start.search')"
          :aria-label="t('sitemap.start.search')"
          data-testid="sitemap-start-search"
        />
        <UButton color="neutral" variant="outline" :label="t('sitemap.start.all')" data-testid="sitemap-start-all" @click="chooseAll(true)" />
        <UButton color="neutral" variant="outline" :label="t('sitemap.start.none')" data-testid="sitemap-start-none" @click="chooseAll(false)" />
      </div>
      <SitemapTree ref="treeView" :tree="tree" :selection="selection" :scope="scope" :version="version" @change="changed" />
      <div class="flex flex-wrap items-start gap-4">
        <UFormField :label="t('sitemap.start.env')" class="min-w-48">
          <USelect v-model="env" :items="envItems" class="w-full" data-testid="sitemap-start-env" />
        </UFormField>
        <UFormField :label="t('sitemap.start.views')">
          <URadioGroup v-model="views" :items="viewItems" orientation="horizontal" data-testid="sitemap-start-views" />
        </UFormField>
      </div>
      <UAlert
        v-if="state.phase === 'refused'"
        color="error"
        variant="subtle"
        role="alert"
        :title="t(failureKey(state.message, 'sitemap.start.refused'))"
        :description="failureDetail(state.message)"
        data-testid="sitemap-start-refused"
      />
      <p v-if="state.phase === 'starting'" role="status" class="text-muted" data-testid="sitemap-start-starting">
        {{ t('sitemap.start.starting') }}
      </p>
      <div class="flex flex-wrap items-center gap-2">
        <span class="tabular-nums" role="status" data-testid="sitemap-start-counter">{{ counter }}</span>
        <span v-if="blocked" class="text-muted" data-testid="sitemap-start-blocked">{{ blocked }}</span>
        <UButton class="ml-auto" color="neutral" variant="ghost" :label="t('sitemap.start.close')" data-testid="sitemap-start-close" @click="start.close()" />
        <UButton :label="t('sitemap.start.start')" :disabled="blocked !== null" data-testid="sitemap-start-start" @click="begin" />
      </div>
    </template>

    <div v-if="state.phase !== 'found' && state.phase !== 'starting' && state.phase !== 'refused'" class="mt-auto flex justify-end">
      <UButton color="neutral" variant="ghost" :label="t('sitemap.start.close')" data-testid="sitemap-start-close" @click="start.close()" />
    </div>
  </main>
</template>
