<script setup lang="ts">
// Settings > Plugins > Catalog (RB-091-01): what the signed repositories offer - the official
// catalogue and the ones the person added (`plugins.catalog.repositories`, below the list) -
// with a search and filters for the kind and for installed plugins and updates. The same plugin
// from two repositories is two rows, each naming its origin. Install and Update first ask the
// core to download and review the package (`catalogPrepare`), then show its full grants as the
// trust dialog does and its publisher; a key not trusted yet, or another key than the installed
// version's, is a step of its own that says the key is trusted for every plugin it signs, and
// warns when an update changed its key - Cancel is the default there (security review H1, M1).
// Only the last confirm sends the install, with exactly what was shown. Remove is offered for what the
// catalogue installed only. A change takes effect at the next start. Every text is rendered as
// text, never as HTML.
import { computed, onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UBadge from '@nuxt/ui/components/Badge.vue';
import UButton from '@nuxt/ui/components/Button.vue';
import UInput from '@nuxt/ui/components/Input.vue';
import UModal from '@nuxt/ui/components/Modal.vue';
import UAlert from '@nuxt/ui/components/Alert.vue';
import USelect from '@nuxt/ui/components/Select.vue';
import TrustGrants from '../../trust/TrustGrants.vue';
import type { IpcClient } from '../../ipc/client';
import type { PanelRequest } from '../../panels/protocol';
import { SETTINGS_ICONS } from '../icons';
import {
  type Catalog,
  type CatalogRow,
  capabilityKey,
  failureKey,
  installRequest,
  keyChanged,
  keyNeedsConfirmation,
  KIND_KEYS,
  KINDS,
  type KindFilter,
  type PackageReview,
  parseCatalog,
  parseReview,
  rowsOf,
  SHOW,
  SHOW_KEYS,
  type ShowFilter,
} from './catalog';

const props = defineProps<{ client: IpcClient }>();
const emit = defineEmits<{ open: [key: string] }>();
const { t, te } = useI18n();

const REPOSITORIES_KEY = 'plugins.catalog.repositories';

const catalog = ref<Catalog | null>(null);
const busy = ref(false);
const failure = ref<string | null>(null);
const notice = ref<string | null>(null);
const query = ref('');
const kind = ref<KindFilter>('all');
const show = ref<ShowFilter>('all');
/** The row whose install is under way, its review, and the dialog's step. */
const confirming = ref<CatalogRow | null>(null);
const review = ref<PackageReview | null>(null);
const step = ref<'review' | 'key'>('review');

const kindItems = computed(() => (['all', ...KINDS] as KindFilter[]).map((value) => ({ value, label: t(KIND_KEYS[value]) })));
const showItems = computed(() => SHOW.map((value) => ({ value, label: t(SHOW_KEYS[value]) })));
const rows = computed(() => (catalog.value === null ? [] : rowsOf(catalog.value, { query: query.value, kind: kind.value, show: show.value })));
const nothingLoaded = computed(() => catalog.value !== null && !catalog.value.repositories.some((repository) => repository.loaded));
const dialogOpen = computed({
  get: () => confirming.value !== null,
  set: (value: boolean) => {
    if (!value) cancel();
  },
});

/** A failure as the person reads it: the core answers with an i18n key. */
function failureText(error: unknown): string {
  const message = error instanceof Error ? error.message : String(error);
  const key = failureKey(message);
  return key !== null && te(key) ? t(key) : t('settings.plugins.catalog.failed');
}

async function ask(request: PanelRequest): Promise<unknown> {
  const answer = await props.client.expect({ type: 'pluginPanel', request }, 'pluginPanel');
  return answer.message;
}

async function load(refresh: boolean): Promise<void> {
  busy.value = true;
  failure.value = null;
  try {
    catalog.value = parseCatalog(await ask({ op: 'catalog', refresh }));
  } catch (error) {
    failure.value = failureText(error);
  } finally {
    busy.value = false;
  }
}

function cancel(): void {
  confirming.value = null;
  review.value = null;
  step.value = 'review';
}

/** Install or Update: the core downloads and reviews the package; nothing is installed yet. */
async function startInstall(row: CatalogRow): Promise<void> {
  busy.value = true;
  failure.value = null;
  try {
    const answer = await ask({ op: 'catalogPrepare', repository: row.offer.repository, id: row.offer.entry.id, version: row.offer.entry.version });
    const reviewed = parseReview(answer);
    if (reviewed === null) {
      failure.value = t('settings.plugins.catalog.failed');
      return;
    }
    review.value = reviewed;
    step.value = 'review';
    confirming.value = row;
  } catch (error) {
    failure.value = failureText(error);
  } finally {
    busy.value = false;
  }
}

/** The review's Install: the key's own step first when the key needs it. */
function confirmReview(): void {
  if (review.value !== null && keyNeedsConfirmation(review.value.key)) {
    step.value = 'key';
    return;
  }
  void sendInstall(false);
}

async function sendInstall(keyConfirmed: boolean): Promise<void> {
  const row = confirming.value;
  const reviewed = review.value;
  if (row === null || reviewed === null) return;
  cancel();
  busy.value = true;
  failure.value = null;
  try {
    await ask(installRequest(reviewed, keyConfirmed));
    notice.value = t('settings.plugins.catalog.installed', { name: row.offer.entry.name });
    await load(false);
  } catch (error) {
    failure.value = failureText(error);
  } finally {
    busy.value = false;
  }
}

async function remove(row: CatalogRow): Promise<void> {
  busy.value = true;
  failure.value = null;
  try {
    await ask({ op: 'catalogRemove', id: row.offer.entry.id });
    notice.value = t('settings.plugins.catalog.removed', { name: row.offer.entry.name });
    await load(false);
  } catch (error) {
    failure.value = failureText(error);
  } finally {
    busy.value = false;
  }
}

function compatibilityText(row: CatalogRow): string | null {
  const compatibility = row.offer.compatibility;
  if (compatibility.state === 'needsNewerApp') {
    return t('settings.plugins.catalog.needs_newer_app', { version: compatibility.minAppVersion });
  }
  if (compatibility.state === 'apiVersion') {
    return t('settings.plugins.catalog.api_version', { version: compatibility.apiVersion });
  }
  return null;
}

onMounted(() => void load(false));
</script>

<template>
  <section class="flex flex-col gap-3" data-testid="plugin-catalog" :aria-busy="busy">
    <div class="flex flex-wrap items-center gap-2">
      <UInput
        v-model="query"
        type="search"
        :icon="SETTINGS_ICONS.search"
        :placeholder="t('settings.plugins.catalog.search')"
        :aria-label="t('settings.plugins.catalog.search')"
        class="min-w-48 flex-1"
        data-testid="catalog-search"
      />
      <USelect v-model="kind" :items="kindItems" :aria-label="t('settings.plugins.catalog.kind.label')" data-testid="catalog-kind" />
      <USelect v-model="show" :items="showItems" :aria-label="t('settings.plugins.catalog.show.label')" data-testid="catalog-show" />
      <UButton
        :icon="SETTINGS_ICONS.refresh"
        :loading="busy"
        variant="outline"
        data-testid="catalog-refresh"
        @click="load(true)"
      >
        {{ t('settings.plugins.catalog.check') }}
      </UButton>
    </div>

    <p v-if="failure !== null" class="text-sm text-error" role="alert" data-testid="catalog-failure">{{ failure }}</p>
    <p v-if="notice !== null" class="text-sm text-success" role="status" data-testid="catalog-notice">
      {{ notice }} {{ t('settings.plugins.catalog.restart') }}
    </p>

    <ul v-if="catalog !== null" class="flex flex-col gap-1 text-sm" data-testid="catalog-repositories">
      <li v-for="repository in catalog.repositories" :key="repository.id" class="flex flex-wrap items-center gap-2 text-muted">
        <span class="font-medium text-default">{{ repository.official ? t('settings.plugins.catalog.official') : repository.name }}</span>
        <span class="truncate">{{ repository.url }}</span>
        <UBadge v-if="repository.error !== null" color="error" variant="subtle">{{ failureText(repository.error) }}</UBadge>
        <UBadge v-else-if="!repository.loaded" color="neutral" variant="subtle">{{ t('settings.plugins.catalog.not_loaded') }}</UBadge>
      </li>
      <li v-for="refused in catalog.refused" :key="refused.url" class="flex flex-wrap items-center gap-2 text-muted">
        <span class="font-medium text-default">{{ refused.name || refused.url }}</span>
        <UBadge color="error" variant="subtle">{{ failureText(refused.error) }}</UBadge>
      </li>
    </ul>

    <p v-if="nothingLoaded" class="text-sm text-muted" data-testid="catalog-empty">{{ t('settings.plugins.catalog.none_loaded') }}</p>
    <p v-else-if="catalog !== null && rows.length === 0" class="text-sm text-muted">{{ t('settings.plugins.catalog.no_match') }}</p>

    <ul class="flex flex-col divide-y divide-default rounded-md border border-default">
      <li
        v-for="row in rows"
        :key="row.key"
        class="flex flex-wrap items-start gap-3 p-3"
        data-testid="catalog-row"
        :data-plugin="row.offer.entry.id"
        :data-repository="row.offer.repository"
      >
        <div class="flex min-w-0 flex-1 flex-col gap-1">
          <div class="flex flex-wrap items-center gap-2">
            <span class="font-medium">{{ row.offer.entry.name }}</span>
            <span class="text-muted">{{ row.offer.entry.version }}</span>
            <UBadge :color="row.severalOrigins ? 'warning' : 'neutral'" variant="subtle" data-testid="catalog-origin">
              {{ t('settings.plugins.catalog.from', { repository: row.repositoryName }) }}
            </UBadge>
            <UBadge v-if="row.bundled" color="neutral" variant="subtle" data-testid="catalog-bundled">
              {{ t('settings.plugins.catalog.bundled') }}
            </UBadge>
            <UBadge v-if="row.installedVersion !== null" color="success" variant="subtle">
              {{ t('settings.plugins.catalog.installed_version', { version: row.installedVersion }) }}
            </UBadge>
            <UBadge v-if="row.update" color="info" variant="subtle">{{ t('settings.plugins.catalog.update') }}</UBadge>
            <UBadge v-if="compatibilityText(row) !== null" color="warning" variant="subtle">{{ compatibilityText(row) }}</UBadge>
          </div>
          <p class="text-muted">{{ row.offer.entry.description }}</p>
        </div>
        <UButton
          v-if="row.action === 'install' || row.action === 'update'"
          :disabled="busy"
          :data-testid="`catalog-${row.action}`"
          @click="startInstall(row)"
        >
          {{ t(row.action === 'install' ? 'settings.plugins.catalog.install' : 'settings.plugins.catalog.update_action') }}
        </UButton>
        <UButton
          v-else-if="row.action === 'remove'"
          color="neutral"
          variant="outline"
          :icon="SETTINGS_ICONS.remove"
          :disabled="busy"
          data-testid="catalog-remove"
          @click="remove(row)"
        >
          {{ t('settings.plugins.catalog.remove') }}
        </UButton>
      </li>
    </ul>

    <UButton variant="link" class="self-start" data-testid="catalog-repositories-key" @click="emit('open', REPOSITORIES_KEY)">
      {{ t('settings.plugins.catalog.manage_repositories') }}
    </UButton>

    <UModal
      v-model:open="dialogOpen"
      :title="confirming === null ? '' : t('settings.plugins.catalog.confirm_title', { name: confirming.offer.entry.name })"
      :description="t(step === 'key' ? 'settings.plugins.catalog.key_description' : 'settings.plugins.catalog.confirm_description')"
      data-testid="catalog-confirm"
    >
      <template #body>
        <div v-if="confirming !== null && review !== null && step === 'review'" class="flex flex-col gap-3 text-sm">
          <p>
            {{ t('settings.plugins.catalog.from', { repository: confirming.repositoryName }) }} -
            {{ review.version }}
          </p>
          <div>
            <p class="font-medium">{{ t('settings.plugins.catalog.capabilities') }}</p>
            <ul class="list-disc ps-5" data-testid="catalog-capabilities">
              <li v-for="capability in review.capabilities" :key="capability">
                {{ te(capabilityKey(capability)) ? t(capabilityKey(capability)) : capability }}
              </li>
              <li v-if="review.capabilities.length === 0">{{ t('settings.plugins.catalog.no_capabilities') }}</li>
            </ul>
          </div>
          <!-- The full grants, as the trust dialog shows a project plugin's (security review M1). -->
          <TrustGrants :grants="review.grants" />
          <div>
            <p class="font-medium">{{ t('settings.plugins.catalog.publisher') }}</p>
            <p>{{ confirming.offer.entry.publisher.author }} ({{ review.key.keyId }})</p>
            <p class="break-all font-mono text-xs text-muted" data-testid="catalog-fingerprint">{{ review.key.fingerprint }}</p>
          </div>
          <p v-if="confirming.offer.entry.releaseNotes !== null" class="whitespace-pre-line text-muted">
            {{ confirming.offer.entry.releaseNotes }}
          </p>
        </div>
        <div v-else-if="review !== null && step === 'key'" class="flex flex-col gap-3 text-sm" data-testid="catalog-key-step">
          <UAlert
            v-if="keyChanged(review.key)"
            color="error"
            variant="subtle"
            :icon="SETTINGS_ICONS.warning"
            :title="t('settings.plugins.catalog.key_changed_title')"
            :description="t('settings.plugins.catalog.key_changed')"
            data-testid="catalog-key-changed"
          />
          <p>{{ t('settings.plugins.catalog.key_every_plugin') }}</p>
          <p>{{ review.key.keyId }}</p>
          <p class="break-all font-mono text-xs" data-testid="catalog-key-fingerprint">{{ review.key.fingerprint }}</p>
          <p v-if="keyChanged(review.key)" class="break-all font-mono text-xs text-muted">
            {{ t('settings.plugins.catalog.key_previous', { fingerprint: review.key.previousFingerprint ?? '' }) }}
          </p>
        </div>
      </template>
      <template #footer>
        <div class="flex flex-wrap justify-end gap-2">
          <UButton
            :variant="step === 'key' ? 'solid' : 'ghost'"
            :autofocus="step === 'key'"
            data-testid="catalog-cancel"
            @click="cancel()"
          >
            {{ t('settings.plugins.catalog.cancel') }}
          </UButton>
          <UButton v-if="step === 'review'" data-testid="catalog-confirm-install" @click="confirmReview()">
            {{ t('settings.plugins.catalog.confirm') }}
          </UButton>
          <UButton
            v-else
            color="error"
            variant="outline"
            data-testid="catalog-trust-key"
            @click="sendInstall(true)"
          >
            {{ t('settings.plugins.catalog.trust_key_and_install') }}
          </UButton>
        </div>
      </template>
    </UModal>
  </section>
</template>
