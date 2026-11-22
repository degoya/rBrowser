<script setup lang="ts">
// The settings (RB-010-08): the categories on the left in their fixed order, the sections of the
// chosen one on the right with their keys by `x-order`, all generated from the schema. Search
// over label, key and description; "only changed"; reset per field and per section; the JSON
// view per scope; a deep link (`#/settings/<key>`) opens the key's category and highlights it, a
// link to an area (`#/settings/<category>?section=<id>`, RB-090-85) the section's tab with the
// section highlighted.
// Each section is a card (`SectionCard`); a category of more than one section shows one section
// per sub-tab, the tab in the address (`#/settings/layout?tab=grid`, RB-080-13, RB-080-71), unless
// `settings.sectionTabs` is off. The search and "Only changed" list their hits across every
// category and tab at once, the tabs hidden - a hit may lie in any category, and one list answers
// "where is it" in one look where a count per tab would only cover the open category; "Show in
// ..." on a hit opens its category and tab. A category reopens at the tab it was left on.
// Plugins has its own tabs (RB-080-80): Installed and Disabled list the plugins with a count
// each, Settings holds the category's cards; with the tabs off the list heads its card.
// The search is a `UInput`, the categories a vertical `UNavigationMenu` and the sandbox warning a
// `UAlert` (Nuxt UI, RB-080-88); in a narrow window the categories are a select above the content
// (`SettingsNav.vue`, RB-080-96).
// A link that opens no setting names its key at the top (`MissingLinkNotice`, RB-094-26), never
// silently another page; the schema is read again when the plugins change, so their keys are
// there, and a link that waited for the plugins is asked for again once they are in.
import { computed, nextTick, onMounted, onUnmounted, provide, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import UAlert from '@nuxt/ui/components/Alert.vue';
import UButton from '@nuxt/ui/components/Button.vue';
import type { NavigationMenuItem } from '@nuxt/ui/components/NavigationMenu.vue';
import type { IpcClient } from '../ipc/client';
import AboutPage from '../about/AboutPage.vue';
import type { DevicePreset, JsonValue } from '../ipc/protocol';
import { isObject } from '../../../plugins/web-common/src/is-object';
import { BREAKPOINTS_KEY, countReferences, REMOVAL_NOTE, removalNote } from './breakpointRefs';
import DevicePresetList from '../presets/DevicePresetList.vue';
import DiagnosticsSave from './DiagnosticsSave.vue';
import TrustPage from '../trust/TrustPage.vue';
import { SETTINGS_ICONS } from './icons';
import JsonPanel from './JsonPanel.vue';
import RefusalNotice from './RefusalNotice.vue';
import MissingLinkNotice from './MissingLinkNotice.vue';
import { createInstalledLoader, isPluginsChanged, refreshOnChange } from './plugins/loader';
import CatalogPanel from './plugins/CatalogPanel.vue';
import PluginsList from './plugins/PluginsList.vue';
import { CATALOG_TAB, countTabs, disabledIds, type PluginListTab, showBundled } from './plugins/pluginTabs';
import { type MissingLink, routeHash } from './route';
import { shownValues } from './secret';
import { type CategoryView, findDef, layoutOf } from './schema';
import { filterOf } from './search';
import SectionCard from './SectionCard.vue';
import SectionHeader from './SectionHeader.vue';
import SettingsNav, { type SettingsCategoryItem } from './SettingsNav.vue';
import { isNarrow, useWindowWidth } from './narrow';
import TabStrip from './TabStrip.vue';
import { flagOf, SECTION_TABS_KEY } from './flags';
import { createSettingsStore } from './store';
import { activeTab, PLUGINS_CATEGORY, rememberedTab, rememberTab, sectionsOfTab, tabOfSection, tabsOf } from './tabs';

const props = defineProps<{
  client: IpcClient;
  routeKey: string | null;
  /** The sub-tab of the category the route opens (`?tab=`), RB-080-13. */
  routeTab?: string | null;
  /** The section the route opens and highlights (`?section=`), RB-090-85. */
  routeSection?: string | null;
  /** The link that opened nothing (`?missing=`, `?pending=`), RB-094-26. */
  routeMissing?: MissingLink | null;
  /** Whether the open project is trusted (Project > workspace trust shows it). */
  trusted?: boolean | null;
  /** The page width, for tests; the window's own width otherwise (RB-080-96). */
  pageWidth?: number;
}>();
const { t, locale } = useI18n();
const store = createSettingsStore(props.client);
// Deleting a breakpoint says which cells and selections go with it (RB-090-61).
provide(REMOVAL_NOTE, (key: string, item: JsonValue) => {
  if (key !== BREAKPOINTS_KEY || !isObject(item) || typeof item.id !== 'string') return null;
  const found = countReferences((name) => store.values.value.get(name)?.value, item.id);
  return removalNote(found, t, locale.value);
});

const query = ref('');
const onlyChanged = ref(false);
const selected = ref('general');
const highlighted = ref<string | null>(null);
/** The section an area link highlights, as `<category>.<section>`. */
const highlightedSection = ref<string | null>(null);
const showJson = ref(false);
/** The wanted sub-tab of the selected category; `null` for its first. */
const tab = ref<string | null>(null);
/** The link that opened no setting, said at the top (RB-094-26); `null` once a place opened. */
const notFound = ref<MissingLink | null>(null);

// The installed plugins, for the plugin tabs and their counts (RB-080-80); asked for when
// Plugins opens, so the list is fresh there.
const plugins = createInstalledLoader(props.client);
// RB-092-06: an install, update or removal takes effect live; a list shown follows it.
let stopFollowing: (() => void) | null = null;
// RB-094-26: the plugins changed - their keys come with the schema read again.
let stopPlugins: (() => void) | null = null;

onMounted(() => {
  void store.start();
  stopFollowing = refreshOnChange(props.client, plugins);
  stopPlugins = props.client.on((message) => {
    if (isPluginsChanged(message)) void followPlugins();
  });
});
onUnmounted(() => {
  store.stop();
  plugins.stop();
  stopFollowing?.();
  stopPlugins?.();
});
watch(selected, (id) => {
  if (id === PLUGINS_CATEGORY) plugins.refresh();
});

// Breakpoints & Devices > Device presets lists the presets in effect (RB-010-12).
const devices = ref<DevicePreset[]>([]);
watch(selected, (id) => {
  if (id !== 'breakpoints') return;
  props.client
    .expect({ type: 'getDevices' }, 'devices')
    .then((answer) => (devices.value = answer.items))
    .catch(() => undefined);
});

const schema = computed(() => store.schema.value);
const hasProject = computed(() => schema.value?.project !== null && schema.value !== null);
/** The label of the setting a refused write was for; its key when the schema has none. */
const refusalLabel = computed(() => {
  const key = store.refusal.value?.key ?? '';
  const def = schema.value === null ? undefined : findDef(schema.value, key);
  return def === undefined ? key : t(def.labelKey);
});
/** The values of the JSON view: the shown keys' only, never a secret key's (RB-094-27). */
const jsonValues = computed(() => shownValues(store.values.value.values(), schema.value?.defs ?? []));
const searching = computed(() => query.value.trim() !== '' || onlyChanged.value);
const allCategories = computed(() => (schema.value === null ? [] : layoutOf(schema.value)));
/** The categories as navigation entries; the open one is the current page unless a search lists hits. */
const navItems = computed<NavigationMenuItem[]>(() =>
  allCategories.value.map((category) => {
    const current = !searching.value && category.id === selected.value;
    return {
      label: t(category.titleKey),
      active: current,
      'aria-current': current ? 'page' : undefined,
      'data-category': category.id,
      onSelect: () => select(category.id),
    };
  }),
);
/** The categories as choices of the narrow window's select (RB-080-96). */
const categoryChoices = computed<SettingsCategoryItem[]>(() =>
  allCategories.value.map((category) => ({ label: t(category.titleKey), value: category.id })),
);
const windowWidth = useWindowWidth();
const narrow = computed(() => isNarrow(props.pageWidth ?? windowWidth.value));
const sectionTabs = computed(() => flagOf(store.values.value, SECTION_TABS_KEY, true));
const visible = computed(() => {
  const current = schema.value;
  if (current === null) {
    return [];
  }
  if (!searching.value) {
    // About has no keys: its page is the About page (RB-010-18), not empty sections.
    return allCategories.value.filter((category) => category.id === selected.value && category.id !== 'about');
  }
  const include = filterOf({ query: query.value, onlyChanged: onlyChanged.value }, store.values.value, t);
  return layoutOf(current, include)
    .map((category) => ({
      ...category,
      sections: category.sections.filter((section) => !section.mirror),
    }))
    .filter((category) => category.sections.length > 0);
});
const tabs = computed(() => {
  const category = allCategories.value.find((c) => c.id === selected.value);
  return searching.value || category === undefined ? [] : tabsOf(category, sectionTabs.value);
});
/** The plugins per list tab, of those the list shows; none while they load. */
const pluginCounts = computed(() => {
  const list = plugins.list.value;
  if (list === null || list.loading) return null;
  const values = store.values.value;
  const shown = list.plugins.filter((plugin) => showBundled(values) || plugin.source !== 'bundled');
  return countTabs(shown, disabledIds(values));
});
const tabItems = computed(() =>
  tabs.value.map((item) => ({
    label: t(item.titleKey),
    value: item.id,
    badge: selected.value === PLUGINS_CATEGORY ? pluginCounts.value?.[item.id as PluginListTab] : undefined,
  })),
);
/** The plugin tab that lists plugins, if one is open (RB-080-80). */
const pluginListTab = computed<PluginListTab | null>(() => {
  const open = selected.value === PLUGINS_CATEGORY ? tabModel.value : undefined;
  return open === 'installed' || open === 'disabled' ? open : null;
});
const tabModel = computed<string | number | undefined>({
  get: () => activeTab(tabs.value, tab.value) ?? undefined,
  set: (id) => chooseTab(String(id)),
});
const resultCount = computed(() =>
  visible.value.reduce(
    (sum, category) => sum + category.sections.reduce((n, section) => n + section.defs.length, 0),
    0,
  ),
);

// Security & Privacy warns while the Chromium sandbox is off (RB-010-04, ADR 0001 Decision 6).
const SANDBOX_KEY = 'engines.chromium.sandbox';
const sandboxOff = computed(() => store.values.value.get(SANDBOX_KEY)?.value === false);

function sectionsShown(category: CategoryView) {
  return searching.value ? category.sections : sectionsOfTab(category, tab.value, sectionTabs.value);
}

/** Opens category `id`, at the tab it was left on unless `wantedTab` names one. */
function select(id: string, wantedTab: string | null = rememberedTab(id)): void {
  selected.value = id;
  tab.value = wantedTab;
  query.value = '';
  onlyChanged.value = false;
  highlighted.value = null;
  highlightedSection.value = null;
  notFound.value = null;
}

/** Shows sub-tab `id` and puts it in the address, so back and forward walk the tabs and a copied
 * address opens the same tab. The first tab is named too: `layout` and `breakpoints` are keys as
 * well as categories, and `#/settings/layout` alone opens the key. */
function chooseTab(id: string): void {
  tab.value = id;
  highlighted.value = null;
  rememberTab(selected.value, id);
  window.location.hash = routeHash({ page: 'settings', key: selected.value, tab: id });
}

/** Opens section `section` of category `category` on its tab and highlights it (RB-090-85). */
async function openSection(category: string, section: string): Promise<void> {
  select(category, tabOfSection(category, section));
  highlightedSection.value = `${category}.${section}`;
  await nextTick();
  document.querySelector(`[data-section="${category}.${section}"]`)?.scrollIntoView?.({ block: 'start' });
}

/** Opens `key` (a setting or a category, with its tab or section); `false` when the schema
 * holds no such place. */
async function openAt(key: string | null, wantedTab: string | null = null, wantedSection: string | null = null): Promise<boolean> {
  const current = schema.value;
  if (current === null) {
    return false;
  }
  if (key === null) {
    return true;
  }
  const category = current.categories.find((each) => each.id === key);
  // With a tab or a section the address names the category, also where a key has the same
  // name (`layout`).
  const def = (wantedTab !== null || wantedSection !== null) && category ? undefined : findDef(current, key);
  if (def === undefined) {
    if (category && wantedSection !== null && category.sections.includes(wantedSection)) {
      await openSection(category.id, wantedSection);
    } else if (category) {
      // A link to a category opens the tab it names, else the first (`settings.open_plugins`
      // opens the plugin list).
      select(key, wantedTab);
    }
    return category !== undefined;
  }
  // A key's tab is its section (`tabs.ts`), in Plugins the Settings tab; a category without
  // tabs ignores it.
  select(def.category, tabOfSection(def.category, def.section));
  highlighted.value = def.key;
  await nextTick();
  const field = document.getElementById(`field-${def.key}`);
  field?.scrollIntoView?.({ block: 'center' });
  const control =
    document.getElementById(`setting-${def.key.replace(/\./g, '-')}`) ??
    field?.querySelector<HTMLElement>('input:not([disabled]), select, textarea, [tabindex="0"]');
  control?.focus();
  return true;
}

/** The route opened last: a schema read again (the plugins changed, RB-094-26) opens the route
 * only while it has found nothing yet, never back from where the person went since. */
let opened: string | null = null;

/** Opens the route's place, or names the link that opened nothing (RB-094-26). */
async function openRoute(): Promise<void> {
  if (schema.value === null) return;
  const missing = props.routeMissing ?? null;
  const key = props.routeKey;
  const route = JSON.stringify([key, props.routeTab ?? null, props.routeSection ?? null, missing]);
  if (route === opened) return;
  if (missing !== null) {
    opened = route;
    notFound.value = missing;
    // Asked until the plugins are in; then the link is asked for again (`followPlugins`).
    if (missing.waiting) plugins.refresh();
    return;
  }
  if (await openAt(key, props.routeTab ?? null, props.routeSection ?? null)) {
    opened = route;
  } else if (key !== null) {
    // A key this page's schema does not hold (yet): said, and opened once a schema read again
    // holds it.
    notFound.value = { key, waiting: false };
  }
}

/** The plugins changed or came in: the schema with their keys is read again, and a link that
 * waited for them is asked for again - the core opens its place now, or names it missing. */
async function followPlugins(): Promise<void> {
  await store.reloadSchema().catch(() => undefined);
  const waiting = notFound.value?.waiting === true ? notFound.value.key : '';
  if (waiting !== '') {
    await props.client.request({ type: 'openSettings', key: waiting }).catch(() => undefined);
  }
}

watch(
  [() => props.routeKey, () => props.routeTab, () => props.routeSection, () => props.routeMissing, schema],
  () => void openRoute(),
  { immediate: true },
);
// A link waiting for the plugins: once the list says they are loaded, it is asked for again.
watch(
  () => plugins.list.value,
  (list) => {
    if (list !== null && !list.loading && notFound.value?.waiting === true) void followPlugins();
  },
);
</script>

<template>
  <!-- overflow-clip, not hidden: scrollIntoView would scroll a hidden root and leave it there. -->
  <div class="flex h-screen overflow-clip bg-default text-default" :class="narrow ? 'flex-col' : ''">
    <SettingsNav
      v-model:query="query"
      v-model:only-changed="onlyChanged"
      :items="navItems"
      :categories="categoryChoices"
      :current="searching ? null : selected"
      :narrow="narrow"
      @select="select"
    />

    <main
      class="flex min-h-0 min-w-0 flex-1 flex-col gap-4 overflow-y-auto"
      :class="narrow ? 'p-3' : 'p-6'"
      data-testid="settings-main"
    >
      <p v-if="store.loadError.value" class="text-sm text-error" role="alert">
        {{ t('settings.load_failed', { reason: store.loadError.value }) }}
      </p>
      <!-- A refused write at the top as well (RB-094-25): its field may lie out of sight. -->
      <RefusalNotice
        v-if="store.refusal.value"
        :refusal="store.refusal.value"
        :label="refusalLabel"
        @show="(key) => openAt(key)"
        @dismiss="store.dismissRefusal()"
      />
      <header class="flex flex-wrap items-end justify-between gap-2">
        <SectionHeader
          level="page"
          :eyebrow="searching ? t('settings.search.eyebrow') : t('settings.eyebrow')"
          :title="searching ? t('settings.search.results', { n: resultCount }) : t(`settings.category.${selected}`)"
        />
        <UButton
          :icon="SETTINGS_ICONS.json"
          color="neutral"
          variant="soft"
          size="sm"
          :label="showJson ? t('settings.json.hide') : t('settings.json.show')"
          :aria-expanded="showJson"
          data-testid="json-toggle"
          @click="showJson = !showJson"
        />
      </header>
      <!-- The hits of a search, said as they change; empty, and so silent, without one (RB-090-22). -->
      <p class="sr-only" role="status" data-testid="search-status">
        {{ searching ? t('settings.search.results', { n: resultCount }) : '' }}
      </p>
      <!-- A link that opened no setting names its key (RB-094-26), never silently another page. -->
      <MissingLinkNotice v-if="notFound" :link="notFound" @dismiss="notFound = null" />
      <JsonPanel v-if="showJson" :values="jsonValues" />

      <p
        v-if="!searching && selected === 'project' && hasProject"
        class="text-sm text-muted"
        data-testid="trust-status"
      >
        {{ trusted ? t('settings.trust.trusted') : t('settings.trust.restricted') }}
      </p>
      <UAlert
        v-if="!searching && selected === 'security' && sandboxOff"
        role="alert"
        color="warning"
        variant="subtle"
        orientation="horizontal"
        :icon="SETTINGS_ICONS.warning"
        :description="t('settings.security.sandbox_off')"
        data-testid="sandbox-warning"
      >
        <template #actions>
          <UButton
            color="warning"
            variant="soft"
            size="xs"
            :label="t('settings.security.sandbox_open')"
            @click="openAt(SANDBOX_KEY)"
          />
        </template>
      </UAlert>
      <!-- Security & Privacy / Project > Workspace trust (RB-010-09). -->
      <TrustPage
        v-if="!searching && (selected === 'security' || (selected === 'project' && hasProject))"
        data-testid="settings-trust"
      />

      <!-- About > version, engines, licences (RB-010-18). -->
      <AboutPage v-if="!searching && selected === 'about'" :client="client" data-testid="settings-about" />

      <TabStrip v-if="tabs.length > 0" v-model="tabModel" :items="tabItems" data-testid="settings-tabs" />
      <!-- Plugins > Installed / Disabled: the plugins of the tab (RB-080-80). -->
      <PluginsList
        v-if="pluginListTab !== null"
        :key="pluginListTab"
        :client="client"
        :store="store"
        :loader="plugins"
        :tab="pluginListTab"
        @open="openAt"
      />

      <!-- Plugins > Catalog: the signed plugin catalogue, above its keys (RB-091-01). -->
      <CatalogPanel
        v-if="!searching && selected === PLUGINS_CATEGORY && tabModel === CATALOG_TAB"
        :client="client"
        @open="openAt"
      />

      <template v-for="category in visible" :key="category.id">
        <p v-if="!searching && category.sections.length === 0" class="text-sm text-muted">
          {{ t('settings.empty_category') }}
        </p>
        <SectionCard
          v-for="section in sectionsShown(category)"
          :key="section.id"
          :client="client"
          :store="store"
          :category="category"
          :section="section"
          :has-project="hasProject"
          :highlighted="highlighted"
          :section-highlighted="highlightedSection === `${category.id}.${section.id}`"
          :searching="searching"
          @open="openAt"
        >
          <template v-if="category.id === 'breakpoints' && section.id === 'device-presets'" #default>
            <DevicePresetList :items="devices" />
          </template>
          <!-- Advanced > Diagnostics: "Save diagnostics...", above its key (RB-080-73). -->
          <template v-if="!searching && category.id === 'advanced' && section.id === 'diagnostics'" #before>
            <DiagnosticsSave :client="client" />
          </template>
          <!-- Plugins > Installed plugins: the plugins themselves, above their keys (RB-080-51). -->
          <template v-if="!searching && tabs.length === 0 && category.id === 'plugins' && section.id === 'installed'" #before>
            <PluginsList :client="client" :store="store" :loader="plugins" @open="openAt" />
          </template>
        </SectionCard>
      </template>
    </main>
  </div>
</template>
