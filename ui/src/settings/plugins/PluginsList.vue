<script setup lang="ts">
// Settings > Plugins > Installed plugins (RB-080-51): every plugin the core found - bundled,
// installed by the person, the project's - with its version, where it comes from, its signature
// and whether it runs, the reason when it did not load, a link to its settings where they hang
// and to its commands in the shortcut editor. The switch writes `plugins.disabled`, which the
// next start reads - only with the id of a verified plugin: a refused or waiting package is
// named by its file, has no switch, and what its manifest claims is marked unverified. The page
// installs and trusts nothing. Every text of a row is rendered as text (interpolation), never
// as HTML. On a plugin tab (RB-080-80) it lists the plugins of that tab only; a row switched
// over leaves it, and the focus goes to the switch now in its place, never to the body.
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import UBadge from '@nuxt/ui/components/Badge.vue';
import UButton from '@nuxt/ui/components/Button.vue';
import USwitch from '@nuxt/ui/components/Switch.vue';
import type { IpcClient } from '../../ipc/client';
import { SETTINGS_ICONS } from '../icons';
import { SHORTCUTS_KEY } from '../schema';
import type { SettingsStore } from '../store';
import { commandQuery, type InstalledPlugin, rowName, SOURCES, switchable } from './installed';
import { restartKey, signatureText, SIGNATURE_COLORS, SOURCE_KEYS, STATE_COLORS, STATE_KEYS } from './labels';
import { createInstalledLoader, type InstalledLoader } from './loader';
import {
  DISABLED_KEY,
  disabledIds,
  PLUGIN_TAB_EMPTY_KEYS,
  type PluginListTab,
  pluginTab,
  showBundled as showBundledOf,
} from './pluginTabs';
import { openShortcutsWith } from './shortcutQuery';

const props = defineProps<{
  client: IpcClient;
  store: SettingsStore;
  /** The page's loader, shared with the tab counts (RB-080-80); without one the list asks
   * the core itself and stops when it is left. */
  loader?: InstalledLoader;
  /** Only the plugins of this tab (RB-080-80); every plugin without. */
  tab?: PluginListTab;
}>();
const emit = defineEmits<{ open: [key: string] }>();
const { t } = useI18n();

const own = props.loader === undefined;
const loader = props.loader ?? createInstalledLoader(props.client);
const { list, loadError, timedOut, failed, refresh } = loader;

// A shared loader is the page's to start: it asks when Plugins opens.
onMounted(() => {
  if (own) {
    refresh();
  }
});
onUnmounted(() => {
  if (own) {
    loader.stop();
  }
});

const disabled = computed<string[]>(() => disabledIds(props.store.values.value));
const showBundled = computed(() => showBundledOf(props.store.values.value));
const all = computed(() => list.value?.plugins ?? []);
const visible = computed(() => all.value.filter((plugin) => showBundled.value || plugin.source !== 'bundled'));
const shown = computed(() =>
  props.tab === undefined ? visible.value : visible.value.filter((plugin) => pluginTab(plugin, disabled.value) === props.tab),
);
const hidden = computed(() => all.value.length - visible.value.length);
const folders = computed(() => SOURCES.flatMap((source) => list.value?.folders[source] ?? []));
const missing = computed(() => SOURCES.flatMap((source) => list.value?.missing[source] ?? []));
/** A tab of its own with none of the plugins there (the others are on the other tab). */
const tabEmpty = computed(() => props.tab !== undefined && all.value.length > 0 && shown.value.length === 0);

/** Per plugin id the first key of its settings, where they hang: once per schema. */
const settingsKeys = computed(() => {
  const first = new Map<string, { key: string; order: number }>();
  for (const def of props.store.schema.value?.defs ?? []) {
    if (def.plugin === undefined) continue;
    const held = first.get(def.plugin);
    if (held === undefined || def.order < held.order || (def.order === held.order && def.key < held.key)) {
      first.set(def.plugin, { key: def.key, order: def.order });
    }
  }
  return new Map([...first].map(([plugin, held]) => [plugin, held.key]));
});

/** What a row shows beside the manifest's facts, worked out once per list, schema and setting. */
const rows = computed(() =>
  shown.value.map((plugin) => ({
    plugin,
    name: rowName(plugin),
    settingsKey: plugin.verified ? (settingsKeys.value.get(plugin.id) ?? null) : null,
    restartKey: restartKey(plugin, disabled.value),
    switchable: switchable(plugin),
  })),
);

function openCommands(plugin: InstalledPlugin): void {
  openShortcutsWith(commandQuery(plugin.id));
  emit('open', SHORTCUTS_KEY);
}

/** Where the focus goes when a switched row leaves its tab: the switch now at its place, else
 * the one before, else the list itself - never the body (RB-080-80). */
const root = ref<HTMLElement | null>(null);
let leaving: { id: string; index: number } | null = null;
watch(rows, async (next) => {
  const gone = leaving;
  if (gone === null || next.some((row) => row.plugin.verified && row.plugin.id === gone.id)) {
    return;
  }
  leaving = null;
  await nextTick();
  const switches = [...(root.value?.querySelectorAll<HTMLElement>('[data-testid="plugin-enabled"]') ?? [])];
  (switches[Math.min(gone.index, switches.length - 1)] ?? root.value)?.focus();
});

/** Writes the id of a verified plugin into or out of `plugins.disabled`; never what a refused
 * package claims. */
function setEnabled(plugin: InstalledPlugin, enabled: boolean): void {
  if (!switchable(plugin)) {
    return;
  }
  if (props.tab !== undefined) {
    const index = rows.value.filter((row) => row.switchable).findIndex((row) => row.plugin.id === plugin.id);
    leaving = { id: plugin.id, index: Math.max(0, index) };
  }
  const rest = disabled.value.filter((id) => id !== plugin.id);
  const next = enabled ? rest : [...rest, plugin.id].sort();
  // `null` resets the key: an empty list is its default.
  void props.store.set(DISABLED_KEY, next.length === 0 ? null : next, 'user');
}
</script>

<template>
  <section
    ref="root"
    tabindex="-1"
    class="flex flex-col gap-2 px-2 py-3 focus-visible:outline-2 focus-visible:outline-primary"
    :aria-label="t('settings.plugins.list')"
    data-testid="plugins-list"
  >
    <div class="flex flex-wrap items-center justify-between gap-2">
      <p v-if="loadError" class="text-sm text-error" role="alert" data-testid="plugins-error">
        {{ t('settings.plugins.load_failed', { reason: loadError }) }}
      </p>
      <p v-else-if="timedOut" class="text-sm text-error" role="alert" data-testid="plugins-error">
        {{ t('settings.plugins.load_timeout') }}
      </p>
      <p v-else-if="list === null || list.loading" class="text-sm text-muted" role="status" data-testid="plugins-loading">
        {{ t('settings.plugins.loading') }}
      </p>
      <p v-else-if="all.length === 0" class="text-sm text-muted" data-testid="plugins-empty">
        {{ t('settings.plugins.empty') }}
        <template v-if="folders.length === 0 && missing.length === 0">{{ t('settings.plugins.empty_no_folder') }}</template>
        <template v-for="folder in folders" :key="folder">
          {{ t('settings.plugins.empty_folder', { folder }) }}
        </template>
        <template v-for="folder in missing" :key="folder">
          {{ t('settings.plugins.missing_folder', { folder }) }}
        </template>
      </p>
      <p v-else-if="tabEmpty && tab !== undefined" class="text-sm text-muted" data-testid="plugins-tab-empty">
        {{ t(PLUGIN_TAB_EMPTY_KEYS[tab]) }}
      </p>
      <p v-else-if="hidden > 0" class="text-sm text-muted" data-testid="plugins-hidden">
        {{ t('settings.plugins.hidden_bundled', { n: hidden }) }}
      </p>
      <span v-else />
      <UButton
        color="neutral"
        :variant="failed ? 'soft' : 'ghost'"
        size="xs"
        :label="failed ? t('settings.plugins.retry') : t('settings.plugins.refresh')"
        data-testid="plugins-refresh"
        @click="refresh"
      />
    </div>

    <ul v-if="rows.length > 0" class="flex flex-col divide-y divide-default">
      <li
        v-for="(row, index) in rows"
        :key="`${index}-${row.plugin.source}-${row.name}`"
        class="flex flex-wrap items-start gap-x-4 gap-y-2 py-3"
        data-testid="plugin-row"
        :data-plugin="row.name"
        :data-state="row.plugin.state"
      >
        <div class="flex min-w-0 flex-1 basis-80 flex-col gap-1">
          <div class="flex flex-wrap items-center gap-2">
            <span class="text-sm font-medium text-highlighted" data-testid="plugin-name">{{ row.plugin.name }}</span>
            <span v-if="row.plugin.verified" class="font-mono text-xs text-muted">{{ row.plugin.id }}</span>
            <span v-if="row.plugin.version" class="text-xs text-muted" data-testid="plugin-version">
              {{ t('settings.plugins.version', { version: row.plugin.version }) }}
            </span>
          </div>
          <div class="flex flex-wrap items-center gap-1">
            <UBadge
              color="neutral"
              variant="subtle"
              size="sm"
              :label="t(SOURCE_KEYS[row.plugin.source])"
              data-testid="plugin-source"
            />
            <UBadge
              :color="SIGNATURE_COLORS[row.plugin.signature.state]"
              variant="subtle"
              size="sm"
              :label="signatureText(row.plugin, t)"
              data-testid="plugin-signature"
              :data-signature="row.plugin.signature.state"
            />
            <UBadge
              :color="STATE_COLORS[row.plugin.state]"
              variant="subtle"
              size="sm"
              :label="t(STATE_KEYS[row.plugin.state])"
              data-testid="plugin-state"
            />
          </div>
          <p v-if="row.plugin.description" class="text-xs text-muted" data-testid="plugin-description">
            {{ row.plugin.description }}
          </p>
          <!-- What a refused package says of itself: unverified, and as text. -->
          <p v-if="row.plugin.declared" class="text-xs text-muted" data-testid="plugin-declared">
            {{ t('settings.plugins.declared', { name: row.plugin.declared.name, version: row.plugin.declared.version }) }}
          </p>
          <p v-if="row.plugin.declared?.claimsIdOf" class="text-xs text-warning" data-testid="plugin-claims">
            {{ t('settings.plugins.claims_id', { name: row.plugin.declared.claimsIdOf }) }}
          </p>
          <p v-if="row.plugin.error" class="text-xs text-error" role="alert" data-testid="plugin-error">
            {{ row.plugin.error.message }} <span class="font-mono">({{ row.plugin.error.code }})</span>
          </p>
          <p v-if="row.restartKey" class="text-xs text-warning" data-testid="plugin-restart">
            {{ t(row.restartKey) }}
          </p>
        </div>
        <div class="flex shrink-0 flex-wrap items-center gap-2">
          <UButton
            v-if="row.settingsKey !== null"
            color="neutral"
            variant="soft"
            size="xs"
            :label="t('settings.plugins.settings')"
            data-testid="plugin-settings"
            @click="emit('open', row.settingsKey)"
          />
          <UButton
            v-if="row.plugin.verified && row.plugin.commands > 0"
            :icon="SETTINGS_ICONS.keyboard"
            color="neutral"
            variant="soft"
            size="xs"
            :label="t('settings.plugins.commands', { n: row.plugin.commands })"
            data-testid="plugin-commands"
            @click="openCommands(row.plugin)"
          />
          <USwitch
            v-if="row.switchable"
            :model-value="!disabled.includes(row.plugin.id)"
            :aria-label="t('settings.plugins.enabled', { name: row.plugin.name })"
            data-testid="plugin-enabled"
            @update:model-value="(on: boolean) => setEnabled(row.plugin, on)"
          />
        </div>
      </li>
    </ul>
  </section>
</template>
