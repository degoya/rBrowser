<script setup lang="ts">
// The setup wizard's project step (RB-091-05): a folder (the native dialog, a WSL folder
// included; the open project without a config is proposed), the dev server's address with the
// usual local ones proposed, and where the breakpoints come from - the analyser's proposal for
// that address, a mobile and a desktop view, or chosen device presets. "Preview" shows the
// config exactly as `rbrowser init` would write it; only "Write" writes it, a person's click.
// A folder that is not the open project becomes a recent project that "Open" starts.
import { computed, onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UAlert from '@nuxt/ui/components/Alert.vue';
import UButton from '@nuxt/ui/components/Button.vue';
import UCheckbox from '@nuxt/ui/components/Checkbox.vue';
import UFormField from '@nuxt/ui/components/FormField.vue';
import URadioGroup from '@nuxt/ui/components/RadioGroup.vue';
import type { DevicePreset, SetupAnswer, SetupProject, SetupTemplate } from '../ipc/protocol';
import { deviceName, deviceSize, groupByKind } from '../presets/devices';
import { SETTINGS_ICONS } from '../settings/icons';
import SuggestInput from '../settings/SuggestInput.vue';
import type { SetupApi } from './api';
import ConfigPreview from './ConfigPreview.vue';
import { DEV_SERVER_ADDRESSES } from './project';

type Proposal = Extract<SetupAnswer, { op: 'proposal' }>;
type Written = Extract<SetupAnswer, { op: 'written' }>;

const props = defineProps<{ api: SetupApi; project?: SetupProject }>();
const emit = defineEmits<{ written: [answer: Written] }>();
const { t, te } = useI18n();

const root = ref(props.project !== undefined && !props.project.hasConfig ? props.project.root : '');
const url = ref('');
const template = ref<SetupTemplate>('default');
const devices = ref<DevicePreset[]>([]);
const chosenDevices = ref<string[]>([]);
const busy = ref(false);
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
    !busy.value &&
    (template.value !== 'analyzer' || url.value.trim() !== '') &&
    (template.value !== 'devices' || chosenDevices.value.length > 0),
);
const fellBack = computed(() => template.value === 'analyzer' && proposal.value?.source === 'default');

function failureText(answer: Extract<SetupAnswer, { op: 'failed' }>): string {
  return te(answer.messageKey) ? t(answer.messageKey, answer.args ?? {}) : t('firstrun.project.error.unknown');
}

function setUrl(text: string): void {
  url.value = text;
  changed();
}

/** The number of the newest preview; an older one's answer is dropped. */
let previewSeq = 0;

function changed(): void {
  // A preview still under way answers for the old input: dropped.
  previewSeq += 1;
  busy.value = false;
  proposal.value = null;
  failure.value = null;
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
  if (!canPreview.value) return;
  busy.value = true;
  failure.value = null;
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

async function write(): Promise<void> {
  if (proposal.value === null) return;
  busy.value = true;
  try {
    const answer = await props.api.write(proposal.value.proposalId);
    if (answer.op === 'written') {
      written.value = answer;
      emit('written', answer);
    } else {
      failure.value = failureText(answer);
    }
  } catch {
    failure.value = t('firstrun.project.error.unknown');
  } finally {
    busy.value = false;
  }
}

function openProject(): void {
  if (written.value !== null) void props.api.runCommand('project.open_recent', { path: written.value.root });
}

onMounted(async () => {
  try {
    devices.value = await props.api.devices();
  } catch {
    devices.value = [];
  }
});
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
        <URadioGroup v-model="template" :items="templates" data-testid="project-template" @update:model-value="changed" />
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
      <div class="flex flex-wrap items-center gap-2">
        <UButton :label="t('firstrun.project.preview')" :disabled="!canPreview" :loading="busy && proposal === null" data-testid="project-preview" @click="preview" />
        <p v-if="root === ''" class="text-xs text-muted">{{ t('firstrun.project.pick_first') }}</p>
      </div>
      <UAlert v-if="failure !== null" color="error" variant="subtle" role="alert" :title="failure" data-testid="project-failed" />
      <template v-if="proposal !== null">
        <UAlert v-if="fellBack" color="info" variant="subtle" :title="t('firstrun.project.fell_back')" data-testid="project-fell-back" />
        <ConfigPreview :config="proposal.config" />
        <div class="flex flex-wrap items-center gap-2">
          <UButton :label="t('firstrun.project.write')" :loading="busy" data-testid="project-write" @click="write" />
          <p class="text-xs text-muted">{{ t('firstrun.project.write_hint', { root: proposal.root }) }}</p>
        </div>
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
