<script setup lang="ts">
// The setup wizard's project step (RB-091-05): a folder (the native dialog, a WSL folder
// included; the open project without a config is proposed), the dev server's address with the
// usual local ones proposed, and where the breakpoints come from - the analyser's proposal for
// that address, a mobile and a desktop view, or chosen device presets. The config shows exactly
// as `rbrowser init` would write it; only the wizard's "Write config & continue" writes it - the
// preview shown last, a person's click - and never over a config. A folder that is not the open
// project becomes a recent project that "Open" starts.
// RB-091-08: the preview runs by itself (debounced) whenever the input changes; with an address
// the analyser is preselected until the person picks a template. Its result says so - "N
// breakpoints from M stylesheets" - or names why it had none (`firstrun.project.reason.<code>`),
// with "Try again" and "Use mobile + desktop"; a failed analysis is never written unasked.
// Without the analyser plugin the reason links to the plugin catalog (RB-094-21).
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UAlert from '@nuxt/ui/components/Alert.vue';
import UButton from '@nuxt/ui/components/Button.vue';
import UCheckbox from '@nuxt/ui/components/Checkbox.vue';
import UFormField from '@nuxt/ui/components/FormField.vue';
import URadioGroup from '@nuxt/ui/components/RadioGroup.vue';
import type { DevicePreset, SetupAnswer, SetupProject, SetupTemplate, ShellToCore } from '../ipc/protocol';
import { deviceName, deviceSize, groupByKind } from '../presets/devices';
import { SETTINGS_ICONS } from '../settings/icons';
import SettingLink from '../settings/SettingLink.vue';
import SuggestInput from '../settings/SuggestInput.vue';
import type { SetupApi } from './api';
import ConfigPreview from './ConfigPreview.vue';
import { DEV_SERVER_ADDRESSES, PREVIEW_DEBOUNCE_MS, proposedViews, reasonKey } from './project';
import type { NextAction } from './steps';

type Proposal = Extract<SetupAnswer, { op: 'proposal' }>;
type Written = Extract<SetupAnswer, { op: 'written' }>;

const props = defineProps<{ api: SetupApi; project?: SetupProject; debounceMs?: number }>();
const emit = defineEmits<{
  written: [answer: Written];
  /** A settings link was clicked (RB-094-21): the wizard opens Settings there. */
  settings: [message: ShellToCore];
}>();
const { t, te } = useI18n();

const root = ref(props.project !== undefined && !props.project.hasConfig ? props.project.root : '');
const url = ref('');
const template = ref<SetupTemplate>('default');
/** The person picked a template: an address no longer preselects the analyser. */
const templateChosen = ref(false);
const devices = ref<DevicePreset[]>([]);
const chosenDevices = ref<string[]>([]);
const busy = ref(false);
/** A preview waits for the input to settle. */
const scheduled = ref(false);
const proposal = ref<Proposal | null>(null);
const written = ref<Written | null>(null);
const failure = ref<string | null>(null);

const templates = computed(() =>
  (['analyzer', 'default', 'devices'] as const).map((value) => ({
    value,
    label: t(`firstrun.project.template.${value}`),
    description: t(`firstrun.project.template.${value}_hint`),
  })),
);
const suggestions = computed(() => DEV_SERVER_ADDRESSES.map((value) => ({ value, label: value })));
const deviceGroups = computed(() => groupByKind(devices.value));
const canPreview = computed(
  () =>
    root.value !== '' &&
    (template.value !== 'analyzer' || url.value.trim() !== '') &&
    (template.value !== 'devices' || chosenDevices.value.length > 0),
);
/** The analyser was asked and had no proposal: its fallback is not written unasked. */
const analyzerFailed = computed(() => template.value === 'analyzer' && proposal.value !== null && proposal.value.source !== 'analyzer');
const writable = computed(() => proposal.value !== null && !analyzerFailed.value);
const found = computed(() =>
  proposal.value?.source === 'analyzer' ? { count: proposedViews(proposal.value.config).length, sheets: proposal.value.sheets ?? 0 } : null,
);
const reasonText = computed(() => {
  const reason = proposal.value?.reason;
  const key = reasonKey(reason?.code);
  const args = { url: url.value.trim(), ...(reason?.params ?? {}) };
  return te(key) ? t(key, args) : t(reasonKey(undefined), args);
});

/** The wizard's footer button on this step. */
const next = computed<NextAction>(() => {
  if (written.value !== null) return { key: 'firstrun.next' };
  if (writable.value) return { key: 'firstrun.project.write_next', busy: busy.value };
  if (busy.value || scheduled.value) return { key: 'firstrun.project.write_next', busy: true };
  return { key: 'firstrun.project.skip_next' };
});

function failureText(answer: Extract<SetupAnswer, { op: 'failed' }>): string {
  return te(answer.messageKey) ? t(answer.messageKey, answer.args ?? {}) : t('firstrun.project.error.unknown');
}

let timer: ReturnType<typeof setTimeout> | null = null;

function cancelTimer(): void {
  if (timer !== null) clearTimeout(timer);
  timer = null;
  scheduled.value = false;
}

/** Previews once the input rests; never for an input it cannot preview. */
function schedule(): void {
  cancelTimer();
  if (!canPreview.value) return;
  scheduled.value = true;
  timer = setTimeout(() => {
    timer = null;
    scheduled.value = false;
    void preview();
  }, props.debounceMs ?? PREVIEW_DEBOUNCE_MS);
}

/** The number of the newest preview; an older one's answer is dropped. */
let previewSeq = 0;

function changed(): void {
  // A preview still under way answers for the old input: dropped.
  previewSeq += 1;
  busy.value = false;
  proposal.value = null;
  failure.value = null;
  schedule();
}

function setUrl(text: string): void {
  url.value = text;
  if (!templateChosen.value) template.value = text.trim() === '' ? 'default' : 'analyzer';
  changed();
}

function chooseTemplate(value: SetupTemplate): void {
  template.value = value;
  templateChosen.value = true;
  changed();
}

async function pick(): Promise<void> {
  const folder = await props.api.pickFolder();
  if (folder !== null) {
    root.value = folder;
    changed();
  }
}

function toggleDevice(id: string, on: boolean | 'indeterminate'): void {
  chosenDevices.value = on === true ? [...chosenDevices.value, id] : chosenDevices.value.filter((entry) => entry !== id);
  changed();
}

async function preview(): Promise<void> {
  cancelTimer();
  if (!canPreview.value) return;
  busy.value = true;
  failure.value = null;
  proposal.value = null;
  previewSeq += 1;
  const mine = previewSeq;
  try {
    const address = url.value.trim();
    const answer = await props.api.preview({
      op: 'preview',
      root: root.value,
      template: template.value,
      ...(address === '' ? {} : { url: address }),
      ...(template.value === 'devices' ? { devices: [...chosenDevices.value] } : {}),
    });
    if (mine !== previewSeq) return;
    if (answer.op === 'proposal') proposal.value = answer;
    else failure.value = failureText(answer);
  } catch {
    if (mine === previewSeq) failure.value = t('firstrun.project.error.unknown');
  } finally {
    if (mine === previewSeq) busy.value = false;
  }
}

/** Writes the preview shown last; `true` when it was written. */
async function write(): Promise<boolean> {
  if (proposal.value === null || !writable.value) return false;
  busy.value = true;
  try {
    const answer = await props.api.write(proposal.value.proposalId);
    if (answer.op === 'written') {
      written.value = answer;
      emit('written', answer);
      return true;
    }
    failure.value = failureText(answer);
    return false;
  } catch {
    failure.value = t('firstrun.project.error.unknown');
    return false;
  } finally {
    busy.value = false;
  }
}

/** The footer button: writes the config shown, or goes on without a project. */
async function proceed(): Promise<boolean> {
  if (written.value !== null) return true;
  if (writable.value) return write();
  return !busy.value && !scheduled.value;
}

function openProject(): void {
  if (written.value !== null) void props.api.runCommand('project.open_recent', { path: written.value.root });
}

defineExpose({ next, proceed });

onMounted(async () => {
  schedule();
  try {
    devices.value = await props.api.devices();
  } catch {
    devices.value = [];
  }
});

onBeforeUnmount(cancelTimer);
</script>

<template>
  <section class="flex flex-col gap-4" data-testid="setup-project" :aria-busy="busy">
    <p class="text-sm text-muted">{{ t('firstrun.project.intro') }}</p>
    <template v-if="written === null">
      <UFormField :label="t('firstrun.project.folder')" :description="t('firstrun.project.folder_hint')">
        <div class="flex flex-wrap items-center gap-2">
          <code class="min-w-0 flex-1 truncate rounded-md bg-elevated px-2 py-1 text-xs" data-testid="project-root">{{
            root === '' ? t('firstrun.project.no_folder') : root
          }}</code>
          <UButton
            :icon="SETTINGS_ICONS.browseFolder"
            color="neutral"
            variant="outline"
            size="sm"
            :label="t('firstrun.project.pick')"
            data-testid="project-pick"
            @click="pick"
          />
        </div>
      </UFormField>
      <UFormField :label="t('firstrun.project.url')" :description="t('firstrun.project.url_hint')">
        <SuggestInput
          :value="url"
          :items="suggestions"
          input-id="setup-url"
          :label="t('firstrun.project.url')"
          :placeholder="DEV_SERVER_ADDRESSES[0]"
          data-testid="project-url"
          @commit="setUrl"
        />
      </UFormField>
      <UFormField :label="t('firstrun.project.template.label')">
        <URadioGroup
          :model-value="template"
          :items="templates"
          :aria-label="t('firstrun.project.template.label')"
          data-testid="project-template"
          @update:model-value="chooseTemplate"
        />
      </UFormField>
      <div v-if="template === 'devices'" class="flex flex-col gap-2" data-testid="project-devices">
        <div v-for="[kind, group] in deviceGroups" :key="kind" class="flex flex-col gap-1">
          <h3 class="text-xs font-semibold uppercase text-muted">{{ t(`firstrun.project.device_kind.${kind}`) }}</h3>
          <UCheckbox
            v-for="device in group"
            :key="device.id"
            :model-value="chosenDevices.includes(device.id)"
            :label="deviceName(device, t)"
            :description="deviceSize(device, t)"
            :data-testid="`device-${device.id}`"
            @update:model-value="(on) => toggleDevice(device.id, on)"
          />
        </div>
      </div>
      <p v-if="root === ''" class="text-xs text-muted" data-testid="project-pick-first">{{ t('firstrun.project.pick_first') }}</p>
      <p v-else-if="busy && proposal === null" class="text-sm text-muted" role="status" data-testid="project-busy">
        {{ template === 'analyzer' ? t('firstrun.project.analysing', { url: url.trim() }) : t('firstrun.project.previewing') }}
      </p>
      <UAlert v-if="failure !== null" color="error" variant="subtle" role="alert" :title="failure" data-testid="project-failed" />
      <UAlert
        v-if="analyzerFailed"
        color="warning"
        variant="subtle"
        role="alert"
        :title="t('firstrun.project.analyzer.failed_title')"
        :description="reasonText"
        data-testid="project-analyzer-failed"
      >
        <template #actions>
          <UButton color="neutral" variant="outline" size="sm" :label="t('firstrun.project.analyzer.retry')" data-testid="project-retry" @click="preview" />
          <UButton size="sm" :label="t('firstrun.project.analyzer.use_default')" data-testid="project-use-default" @click="chooseTemplate('default')" />
          <SettingLink
            v-if="proposal?.reason?.code === 'plugin_missing'"
            setting-key="plugins.catalog"
            @open="emit('settings', $event)"
          />
        </template>
      </UAlert>
      <template v-if="writable && proposal !== null">
        <UAlert
          v-if="found !== null"
          color="success"
          variant="subtle"
          :title="t('firstrun.project.analyzer.found', { count: found.count, sheets: found.sheets })"
          data-testid="project-found"
        />
        <ConfigPreview :config="proposal.config" />
        <p class="text-xs text-muted">{{ t('firstrun.project.write_hint', { root: proposal.root }) }}</p>
      </template>
    </template>
    <div v-else class="flex flex-col gap-2" data-testid="project-written">
      <UAlert color="success" variant="subtle" :title="t('firstrun.project.written', { path: written.path })" />
      <div v-if="!written.open" class="flex flex-wrap items-center gap-2">
        <UButton :label="t('firstrun.project.open')" variant="outline" data-testid="project-open" @click="openProject" />
        <p class="text-xs text-muted">{{ t('firstrun.project.open_hint') }}</p>
      </div>
      <p v-else class="text-sm text-muted">{{ t('firstrun.project.reloaded') }}</p>
    </div>
  </section>
</template>
