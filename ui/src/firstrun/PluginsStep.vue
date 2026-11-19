<script setup lang="ts">
// The setup wizard's plugin step (RB-091-05, after rDownloader's "Your services"): the
// installable first-party plugins by category with a category filter and a search, each with
// its description, size, origin and grants; the recommended and the installed ones ticked, any
// of them can be unticked, "All" and "None" for what is shown. The wizard only marks what gets
// loaded: one confirmation names what is installed with its grants (first-party, the built-in
// key) and what is removed (rDownloader RD-180-14), then `installChosen` with a progress bar
// and `removeChosen`; a failed plugin is named, the others go on. Without a network the core
// installs from the bundle in the package and the step says so. Skipping the step leaves the
// recommended set to the core (`applyRecommended`, the wizard's part). Every control is Nuxt UI.
import { computed, onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UAlert from '@nuxt/ui/components/Alert.vue';
import UBadge from '@nuxt/ui/components/Badge.vue';
import UButton from '@nuxt/ui/components/Button.vue';
import UCheckbox from '@nuxt/ui/components/Checkbox.vue';
import UInput from '@nuxt/ui/components/Input.vue';
import UProgress from '@nuxt/ui/components/Progress.vue';
import USelect from '@nuxt/ui/components/Select.vue';
import { SETTINGS_ICONS } from '../settings/icons';
import { capabilityKey } from '../settings/plugins/catalog';
import type { SetupApi } from './api';
import {
  type Available,
  CATEGORIES,
  changesOf,
  type CategoryFilter,
  grantsOf,
  groupByCategory,
  initialChoice,
  type InstallOutcome,
  type InstallProgress,
  megabytes,
  type PluginsDone,
  totalBytes,
  visiblePlugins,
  withAll,
  withNone,
} from './plugins';

const props = defineProps<{ api: SetupApi }>();
const emit = defineEmits<{ installed: [outcome: PluginsDone]; loaded: [available: Available] }>();
const { t, te } = useI18n();

const available = ref<Available | null>(null);
const loading = ref(true);
const failed = ref(false);
const chosen = ref<string[]>([]);
const query = ref('');
const category = ref<CategoryFilter>('all');
const confirming = ref(false);
const progress = ref<InstallProgress | null>(null);
const installing = ref(false);
const outcome = ref<PluginsDone | null>(null);

const plugins = computed(() => available.value?.plugins ?? []);
const visible = computed(() => visiblePlugins(plugins.value, category.value, query.value));
const groups = computed(() => groupByCategory(visible.value));
const grants = computed(() => grantsOf(plugins.value, chosen.value));
const size = computed(() => megabytes(totalBytes(plugins.value, chosen.value)));
const categoryItems = computed(() =>
  (['all', ...CATEGORIES] as CategoryFilter[]).map((value) => ({ value, label: t(`firstrun.plugins.category.${value}`) })),
);
const percent = computed(() =>
  progress.value === null || progress.value.total === 0 ? 0 : Math.round((progress.value.done / progress.value.total) * 100),
);
const changes = computed(() => changesOf(plugins.value, chosen.value));
const failures = computed(() => (outcome.value?.results ?? []).filter((result) => !result.ok));
const installedCount = computed(() => (outcome.value?.results ?? []).filter((result) => result.ok).length);

function nameOf(id: string): string {
  return plugins.value.find((plugin) => plugin.id === id)?.name ?? id;
}

function errorText(key: string | undefined): string {
  return key !== undefined && te(key) ? t(key) : t('firstrun.plugins.install_failed_unknown');
}

async function load(): Promise<void> {
  loading.value = true;
  failed.value = false;
  try {
    available.value = await props.api.available();
    chosen.value = initialChoice(available.value);
    emit('loaded', available.value);
  } catch {
    failed.value = true;
  } finally {
    loading.value = false;
  }
}

function toggle(id: string, on: boolean | 'indeterminate'): void {
  chosen.value = on === true ? [...chosen.value.filter((entry) => entry !== id), id] : chosen.value.filter((entry) => entry !== id);
}

/** Installs the newly ticked and removes the unticked installed ones; the core always gets the
 * install answer, an empty one included, so a pending first choice is decided. */
async function apply(): Promise<void> {
  confirming.value = false;
  installing.value = true;
  const { install, remove } = changes.value;
  progress.value = { id: install[0] ?? '', done: 0, total: install.length, state: 'installing' };
  let installed: InstallOutcome;
  try {
    installed = await props.api.installChosen(install, (next) => {
      progress.value = next;
    });
  } catch {
    installed = { results: install.map((id) => ({ id, ok: false, origin: 'bundle' })), restart: false };
  }
  let removed: string[] = [];
  if (remove.length > 0) {
    try {
      const taken = await props.api.removeChosen(remove);
      removed = taken.results.filter((result) => result.ok).map((result) => result.id);
      installed = { results: [...installed.results, ...taken.results.filter((result) => !result.ok)], restart: true };
    } catch {
      installed = { ...installed, results: [...installed.results, ...remove.map((id) => ({ id, ok: false, origin: 'bundle' as const }))] };
    }
  }
  outcome.value = { ...installed, removed };
  emit('installed', outcome.value);
  installing.value = false;
}

onMounted(() => void load());
</script>

<template>
  <section class="flex flex-col gap-4" data-testid="setup-plugins" :aria-busy="loading || installing">
    <p class="text-sm text-muted">{{ t('firstrun.plugins.intro') }}</p>
    <p v-if="loading" class="text-sm text-muted" data-testid="plugins-loading">{{ t('firstrun.plugins.loading') }}</p>
    <UAlert
      v-else-if="failed"
      color="error"
      variant="subtle"
      role="alert"
      :title="t('firstrun.plugins.failed')"
      :description="t('firstrun.plugins.failed_hint')"
      data-testid="plugins-failed"
    >
      <template #actions>
        <UButton color="neutral" variant="outline" size="sm" :label="t('firstrun.plugins.retry')" data-testid="plugins-retry" @click="load" />
      </template>
    </UAlert>
    <p v-else-if="plugins.length === 0" class="text-sm text-muted" data-testid="plugins-empty">{{ t('firstrun.plugins.none') }}</p>
    <template v-else>
      <UAlert
        v-if="available?.offline"
        color="info"
        variant="subtle"
        :icon="SETTINGS_ICONS.offline"
        :title="t('firstrun.plugins.offline')"
        data-testid="plugins-offline"
      />
      <div class="flex flex-wrap items-center gap-2">
        <UInput
          v-model="query"
          type="search"
          :icon="SETTINGS_ICONS.search"
          :placeholder="t('firstrun.plugins.search')"
          :aria-label="t('firstrun.plugins.search')"
          class="min-w-48 flex-1"
          data-testid="plugins-search"
        />
        <USelect v-model="category" :items="categoryItems" :aria-label="t('firstrun.plugins.category.label')" data-testid="plugins-category" />
        <UButton color="neutral" variant="ghost" size="sm" :label="t('firstrun.plugins.all')" data-testid="plugins-all" @click="chosen = withAll(chosen, visible)" />
        <UButton color="neutral" variant="ghost" size="sm" :label="t('firstrun.plugins.none_button')" data-testid="plugins-none" @click="chosen = withNone(chosen, visible)" />
      </div>
      <p v-if="visible.length === 0" class="text-sm text-muted" data-testid="plugins-no-match">{{ t('firstrun.plugins.no_match') }}</p>
      <div v-for="[group, entries] in groups" :key="group" class="flex flex-col gap-2" :data-testid="`plugins-group-${group}`">
        <h3 class="text-sm font-semibold text-highlighted">{{ t(`firstrun.plugins.category.${group}`) }}</h3>
        <div v-for="plugin in entries" :key="plugin.id" class="flex flex-col gap-1 rounded-md border border-default p-3">
          <UCheckbox
            :model-value="chosen.includes(plugin.id)"
            :disabled="installing"
            :label="plugin.name"
            :description="plugin.description"
            :data-testid="`plugin-${plugin.id}`"
            @update:model-value="(on) => toggle(plugin.id, on)"
          />
          <div class="flex flex-wrap items-center gap-1 ps-6">
            <UBadge v-if="plugin.installed" color="success" variant="subtle" size="sm" :label="t('firstrun.plugins.installed_badge')" />
            <UBadge v-else-if="plugin.recommended" color="primary" variant="subtle" size="sm" :label="t('firstrun.plugins.recommended')" />
            <UBadge color="neutral" variant="outline" size="sm" :label="t('firstrun.plugins.size', { mb: megabytes(plugin.sizeBytes) })" />
            <UBadge color="neutral" variant="outline" size="sm" :label="t(`firstrun.plugins.origin.${plugin.origin}`)" />
            <UBadge v-for="grant in plugin.grants" :key="grant" color="neutral" variant="soft" size="sm" :label="t(capabilityKey(grant))" />
          </div>
        </div>
      </div>
      <div v-if="outcome === null" class="flex flex-wrap items-center gap-3 border-t border-default pt-3">
        <p class="text-sm text-muted" data-testid="plugins-total">
          {{ t('firstrun.plugins.total', { count: changes.install.length, mb: size }) }}
          <template v-if="changes.remove.length > 0">{{ t('firstrun.plugins.total_remove', { count: changes.remove.length }) }}</template>
        </p>
        <UButton
          :icon="SETTINGS_ICONS.install"
          :label="t('firstrun.plugins.apply')"
          :disabled="installing"
          :loading="installing"
          data-testid="plugins-install"
          @click="confirming = true"
        />
      </div>
      <UAlert
        v-if="confirming"
        color="warning"
        variant="subtle"
        :title="t('firstrun.plugins.confirm_title', { count: changes.install.length })"
        data-testid="plugins-confirming"
      >
        <template #description>
          <p>{{ t('firstrun.plugins.confirm_text', { grants: grants.map((grant) => t(capabilityKey(grant))).join(', ') || t('firstrun.plugins.no_grants') }) }}</p>
          <p v-if="changes.remove.length > 0" data-testid="plugins-confirm-remove">
            {{ t('firstrun.plugins.confirm_remove', { count: changes.remove.length, names: changes.remove.map(nameOf).join(', ') }) }}
          </p>
        </template>
        <template #actions>
          <UButton size="sm" :label="t('firstrun.plugins.confirm')" data-testid="plugins-confirm" @click="apply" />
          <UButton color="neutral" variant="outline" size="sm" :label="t('firstrun.plugins.cancel')" data-testid="plugins-cancel" @click="confirming = false" />
        </template>
      </UAlert>
      <div v-if="installing && progress !== null" class="flex flex-col gap-1" data-testid="plugins-progress">
        <UProgress :model-value="percent" />
        <p class="text-xs text-muted">{{ t('firstrun.plugins.progress', { done: progress.done, total: progress.total, name: nameOf(progress.id) }) }}</p>
      </div>
      <div v-if="outcome !== null" class="flex flex-col gap-2" data-testid="plugins-outcome">
        <p class="text-sm">{{ t('firstrun.plugins.done', { count: installedCount }) }}</p>
        <p v-if="outcome.removed.length > 0" class="text-sm">{{ t('firstrun.plugins.removed', { count: outcome.removed.length }) }}</p>
        <p v-if="outcome.restart" class="text-sm text-muted">{{ t('firstrun.plugins.restart') }}</p>
        <UAlert
          v-for="result in failures"
          :key="result.id"
          color="error"
          variant="subtle"
          role="alert"
          :title="t('firstrun.plugins.install_failed', { name: nameOf(result.id) })"
          :description="errorText(result.error)"
          :data-testid="`plugins-failed-${result.id}`"
        />
      </div>
    </template>
  </section>
</template>
