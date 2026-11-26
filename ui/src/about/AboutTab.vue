<script setup lang="ts">
// The About tab: logo (light and dark, switched by the `dark` class the theme sets), the build,
// the engines, the API versions, the folders, "Copy version info", the links, the author, the
// update status, and the copyright and no-warranty notice of the GPL (RB-099-12). An absent
// Firefox links to Settings > Engines. A link opens in the grid's views (`navigate`) or goes to
// the clipboard; only http(s) addresses do either. The update status is the core's (`updates`):
// not in this build, off, by hand or at every start.
import { computed, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import logoLight from '@assets/rBrowser-logo.svg';
import logoDark from '@assets/rBrowser-logo-dark.svg';
import type { IpcClient } from '../ipc/client';
import type { AboutInfo, AboutUpdates } from '../ipc/protocol';
import { SETTINGS_ICONS } from '../settings/icons';
import {
  apiRows,
  AUTHOR,
  COPYRIGHT_YEAR,
  engineRows,
  engineText,
  isWebLink,
  LICENSE,
  LINKS,
  versionInfo,
} from './facts';

const props = defineProps<{ client: IpcClient; info: AboutInfo }>();
const { t } = useI18n();
const note = ref<{ key: string; error: boolean } | null>(null);

const build = computed(() => [
  { key: 'settings.about.version', value: props.info.version },
  { key: 'settings.about.commit', value: props.info.commit },
  { key: 'settings.about.built', value: props.info.built ?? t('settings.about.unknown') },
  { key: 'settings.about.platform', value: props.info.platform ?? t('settings.about.unknown') },
]);
const engines = computed(() => engineRows(props.info));
const apis = computed(() => apiRows(props.info));
const DIR_KEYS: Record<string, string> = {
  data: 'settings.about.dir.data',
  local: 'settings.about.dir.local',
  logs: 'settings.about.dir.logs',
};
const folders = computed(() =>
  Object.entries(props.info.dirs ?? {}).map(([name, path]) => ({ key: DIR_KEYS[name] ?? name, path })),
);
/** How updates come (RB-091-05); a build without self-update keeps its sentence. */
const UPDATES_KEYS: Record<AboutUpdates, string> = {
  notBuilt: 'settings.about.updates_disabled',
  off: 'settings.about.updates_state.off',
  manual: 'settings.about.updates_state.manual',
  onStart: 'settings.about.updates_state.onStart',
};
const LINK_KEYS = {
  source: 'settings.about.link.source',
  handbook: 'settings.about.link.handbook',
  changelog: 'settings.about.link.changelog',
} as const;

async function copy(text: string, doneKey: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(text);
    note.value = { key: doneKey, error: false };
  } catch {
    note.value = { key: 'settings.about.copy_failed', error: true };
  }
}

function copyVersion(): void {
  void copy(versionInfo(props.info, t), 'settings.about.version_copied');
}

function openLink(url: string): void {
  if (isWebLink(url)) {
    void props.client.request({ type: 'navigate', url }).catch(() => undefined);
  }
}

function copyLink(url: string): void {
  if (isWebLink(url)) {
    void copy(url, 'settings.about.link_copied');
  }
}

function openEngines(): void {
  void props.client.request({ type: 'openSettings', key: 'engines' }).catch(() => undefined);
}
</script>

<template>
  <div class="flex flex-col gap-6" data-testid="about-tab">
    <header class="flex flex-wrap items-center gap-4">
      <img :src="logoLight" :alt="t('settings.about.logo')" class="h-10 w-auto dark:hidden" data-testid="logo-light" />
      <img :src="logoDark" :alt="t('settings.about.logo')" class="hidden h-10 w-auto dark:block" data-testid="logo-dark" />
      <UButton
        :icon="SETTINGS_ICONS.copy"
        color="primary"
        variant="soft"
        size="sm"
        :label="t('settings.about.copy_version')"
        data-testid="copy-version"
        @click="copyVersion"
      />
      <p v-if="note" :class="note.error ? 'text-error' : 'text-muted'" class="text-sm" role="status">
        {{ t(note.key) }}
      </p>
    </header>

    <section class="flex flex-col gap-2" data-section="about.version">
      <h3 class="text-sm font-semibold text-highlighted">{{ t('settings.section.about.version') }}</h3>
      <dl class="grid grid-cols-[max-content_1fr] gap-x-6 gap-y-1 text-sm">
        <template v-for="row in build" :key="row.key">
          <dt class="text-muted">{{ t(row.key) }}</dt>
          <dd class="font-mono">{{ row.value }}</dd>
        </template>
      </dl>
    </section>

    <section class="flex flex-col gap-2" data-section="about.engines">
      <h3 class="text-sm font-semibold text-highlighted">{{ t('settings.section.about.engines') }}</h3>
      <dl class="grid grid-cols-[max-content_1fr] gap-x-6 gap-y-1 text-sm">
        <template v-for="row in engines" :key="row.id">
          <dt class="text-muted">{{ row.name }}</dt>
          <dd class="flex flex-wrap items-center gap-2" :data-engine="row.id">
            <span class="font-mono">{{ engineText(row, t) }}</span>
            <span v-if="row.id === 'firefox' && info.firefoxPath" class="text-muted">{{ info.firefoxPath }}</span>
            <UButton
              v-if="row.id === 'firefox' && !row.version"
              color="neutral"
              variant="link"
              size="xs"
              :label="t('settings.about.open_engines')"
              data-testid="open-engines"
              @click="openEngines"
            />
          </dd>
        </template>
        <template v-if="apis.length > 0">
          <dt class="col-span-2 pt-2 font-semibold text-highlighted">{{ t('settings.about.apis') }}</dt>
          <template v-for="row in apis" :key="row.key">
            <dt class="text-muted">{{ t(row.key) }}</dt>
            <dd class="font-mono">{{ row.value }}</dd>
          </template>
        </template>
      </dl>
    </section>

    <section v-if="folders.length > 0" class="flex flex-col gap-2">
      <h3 class="text-sm font-semibold text-highlighted">{{ t('settings.about.folders') }}</h3>
      <dl class="grid grid-cols-[max-content_1fr] gap-x-6 gap-y-1 text-sm">
        <template v-for="row in folders" :key="row.key">
          <dt class="text-muted">{{ t(row.key) }}</dt>
          <dd class="font-mono break-all">{{ row.path }}</dd>
        </template>
      </dl>
    </section>

    <section class="flex flex-col gap-2">
      <h3 class="text-sm font-semibold text-highlighted">{{ t('settings.about.links') }}</h3>
      <ul class="flex flex-col gap-1 text-sm">
        <li v-for="link in LINKS" :key="link.kind" class="flex flex-wrap items-center gap-2" :data-link="link.kind">
          <span class="min-w-32">{{ t(LINK_KEYS[link.kind]) }}</span>
          <span class="font-mono text-muted break-all">{{ link.url }}</span>
          <span v-if="!link.published" class="text-xs text-muted">({{ t('settings.about.unpublished') }})</span>
          <UButton
            color="neutral"
            variant="ghost"
            size="xs"
            :label="t('settings.about.open_link')"
            data-testid="open-link"
            @click="openLink(link.url)"
          />
          <UButton
            :icon="SETTINGS_ICONS.copy"
            color="neutral"
            variant="ghost"
            size="xs"
            :label="t('settings.about.copy_link')"
            data-testid="copy-link"
            @click="copyLink(link.url)"
          />
        </li>
      </ul>
    </section>

    <section class="flex flex-col gap-1 text-sm">
      <p><span class="text-muted">{{ t('settings.about.author') }}:</span> {{ AUTHOR }}</p>
      <p><span class="text-muted">{{ t('settings.about.license') }}:</span> {{ LICENSE }}</p>
      <p v-if="info.updates !== undefined" data-testid="about-updates">
        <span class="text-muted">{{ t('settings.about.updates') }}:</span> {{ t(UPDATES_KEYS[info.updates]) }}
      </p>
      <p class="pt-2 text-muted">{{ t('settings.about.thanks') }}</p>
      <p class="text-muted" data-testid="about-legal">
        {{ t('settings.about.legal', { year: COPYRIGHT_YEAR, author: AUTHOR }) }}
      </p>
    </section>
  </div>
</template>
