<script setup lang="ts">
// The setup wizard (RB-091-05), the route `setup` of the pages window: welcome, plugins,
// project, basics, done - optional, every step skippable. "Skip setup" on every page and
// "Finish" close the window; the browser is fully usable without it. The core opens it by
// itself once at the very first start, the command `app.setup_wizard` again; the hint "Set up
// project" opens it at the project step (`#/setup/project`). With `ui.firstRunWizard` off it
// says so and offers only Close. Every control is Nuxt UI; each step is a component of its own.
// RB-091-08: one button per step, in the footer - the step names what it does ("Install 3 plugins
// & continue", "Write config & continue", "Apply & continue") and carries it out; only then the
// wizard goes on. Passing a step any other way (the stepper, Back) does nothing.
// RB-091-09: before the end the recommended step "Connect an AI agent" (`AgentStep`), skippable,
// for the project set up in the project step, else the open one; `#/setup/agent` opens it.
// RB-094-21: every text that names a settings path shows a link that opens Settings at its key.
import { computed, onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UAlert from '@nuxt/ui/components/Alert.vue';
import UButton from '@nuxt/ui/components/Button.vue';
import UStepper from '@nuxt/ui/components/Stepper.vue';
import type { IpcClient } from '../ipc/client';
import type { SetupAnswer, SetupProject, ShellToCore } from '../ipc/protocol';
import { SETTINGS_ICONS } from '../settings/icons';
import SettingLink from '../settings/SettingLink.vue';
import { createSetupApi } from './api';
import { createMcpSetupApi } from '../mcpsetup/api';
import AgentStep from './AgentStep.vue';
import BasicsStep from './BasicsStep.vue';
import PluginsStep from './PluginsStep.vue';
import ProjectStep from './ProjectStep.vue';
import { type NextAction, nextStep, previousStep, settingsHash, startStep, type StepControl, STEPS, type StepId } from './steps';
import { type AgentDone, type BasicsChoice, EMPTY_SUMMARY, SETTINGS_PLACES, type SetupSummary, summaryLines } from './summary';
import type { Available, PluginsDone } from './plugins';

const props = defineProps<{ client: IpcClient; routeKey?: string | null }>();
const { t } = useI18n();
const api = createSetupApi(props.client);
const mcpApi = createMcpSetupApi(props.client);

const step = ref<StepId>(startStep(props.routeKey ?? null));
const enabled = ref<boolean | null>(null);
const project = ref<SetupProject | undefined>(undefined);
const summary = ref<SetupSummary>({ ...EMPTY_SUMMARY });

const STEP_ICONS: Record<StepId, string> = {
  welcome: SETTINGS_ICONS.welcome,
  plugins: SETTINGS_ICONS.plugins,
  project: SETTINGS_ICONS.project,
  basics: SETTINGS_ICONS.basics,
  agent: SETTINGS_ICONS.agent,
  done: SETTINGS_ICONS.done,
};
const items = computed(() => STEPS.map((value) => ({ value, title: t(`firstrun.step.${value}`), icon: STEP_ICONS[value] })));
/** Where the basics' layout and sync go: the project written here, else the open one with a config. */
const basicsTarget = computed(() => {
  if (summary.value.project !== null) return summary.value.project.root;
  return project.value?.hasConfig === true ? project.value.root : null;
});
/** Where the agent step's project file goes: the project written here, else the open one. */
const agentRoot = computed(() => summary.value.project?.root ?? project.value?.root ?? null);
const lines = computed(() => summaryLines(summary.value));

function go(next: StepId): void {
  step.value = next;
}

/** The plugin step answered (installed, removed, or kept the choice as it was). */
const pluginsDecided = ref(false);
/** The hint opened the project step: the plugins were decided long ago, nothing is applied. */
const fromHint = props.routeKey === 'project';

/** Whether the first plugin choice is pending (`available`'s `choicePending`); `null` until the
 * plugin step or a skip asked. */
const choicePending = ref<boolean | null>(null);

/** The whole setup was skipped - "Skip setup", "Just start", "Finish" - without the plugin step's
 * answer: while a fresh install's choice is pending (RB-091-04), the core installs its recommended
 * set, once. A choice made before is left as it is. The plugin step's own button never comes here:
 * it installs the ticks (RB-091-08). */
async function skipPlugins(): Promise<void> {
  if (pluginsDecided.value || fromHint) return;
  pluginsDecided.value = true;
  const pending =
    choicePending.value ?? (await api.available().then((available) => available.choicePending).catch(() => false));
  if (pending) await api.applyRecommended().catch(() => undefined);
}

function pluginsLoaded(available: Available): void {
  choicePending.value = available.choicePending;
}

/** The step shown, as it controls the footer button. */
const current = ref<StepControl | null>(null);
const advancing = ref(false);
const nextAction = computed<NextAction>(() => current.value?.next ?? { key: 'firstrun.next' });

/** The footer button: the step carries out what it says, then the wizard goes on. */
async function advance(): Promise<void> {
  if (advancing.value) return;
  advancing.value = true;
  try {
    const from = step.value;
    const goOn = current.value === null ? true : await current.value.proceed();
    if (goOn && step.value === from) go(nextStep(from));
  } finally {
    advancing.value = false;
  }
}

async function close(): Promise<void> {
  if (enabled.value !== false) await skipPlugins();
  await api.close().catch(() => undefined);
}

function openSetting(key: string, tab: string | null): void {
  window.location.hash = settingsHash(key, tab);
}

/** A settings link of a text (RB-094-21): the core opens Settings at its key. */
function openSettings(message: ShellToCore): void {
  void props.client.request(message).catch(() => undefined);
}

function pluginsDone(outcome: PluginsDone): void {
  pluginsDecided.value = true;
  summary.value = { ...summary.value, plugins: outcome };
}

function projectDone(answer: Extract<SetupAnswer, { op: 'written' }>): void {
  summary.value = { ...summary.value, project: answer };
}

function basicsDone(choice: BasicsChoice): void {
  summary.value = { ...summary.value, basics: choice };
}

function agentDone(outcome: AgentDone): void {
  summary.value = { ...summary.value, agent: outcome };
}

onMounted(async () => {
  try {
    const info = await api.info();
    enabled.value = info.enabled;
    project.value = info.project;
  } catch {
    enabled.value = true;
  }
});
</script>

<template>
  <!-- The pages window does not scroll its body (assets/main.css): the wizard scrolls itself, the
       footer with Back and Next stays in sight at the bottom. -->
  <div class="h-screen overflow-y-auto bg-default" data-testid="setup-scroll">
  <main class="mx-auto flex max-w-3xl flex-col gap-6 p-6" data-testid="setup-wizard">
    <header class="flex flex-wrap items-center justify-between gap-2">
      <h1 class="text-lg font-semibold text-highlighted">{{ t('firstrun.title') }}</h1>
      <UButton color="neutral" variant="ghost" :label="t('firstrun.skip')" data-testid="setup-skip" @click="close" />
    </header>
    <UAlert
      v-if="enabled === false"
      color="neutral"
      variant="subtle"
      :title="t('firstrun.off.title')"
      :description="t('firstrun.off.text')"
      data-testid="setup-off"
    >
      <template #actions>
        <div data-testid="setup-off-setting">
          <SettingLink setting-key="ui.firstRunWizard" @open="openSettings" />
        </div>
      </template>
    </UAlert>
    <template v-else>
      <UStepper v-model="step" :items="items" :linear="false" size="sm" data-testid="setup-stepper" />
      <section v-if="step === 'welcome'" class="flex flex-col gap-4" data-testid="setup-welcome">
        <p class="text-base">{{ t('firstrun.welcome.text') }}</p>
        <div class="flex flex-col items-start gap-1">
          <p class="text-sm text-muted">{{ t('firstrun.welcome.optional') }}</p>
          <SettingLink setting-key="ui.firstRunWizard" @open="openSettings" />
        </div>
        <div class="flex flex-wrap gap-2">
          <UButton :icon="SETTINGS_ICONS.wizard" :label="t('firstrun.welcome.setup')" data-testid="setup-start" @click="go('plugins')" />
          <UButton color="neutral" variant="outline" :label="t('firstrun.welcome.just_start')" data-testid="setup-just-start" @click="close" />
        </div>
      </section>
      <PluginsStep
        v-else-if="step === 'plugins'"
        ref="current"
        :api="api"
        @installed="pluginsDone"
        @loaded="pluginsLoaded"
        @settings="openSettings"
      />
      <ProjectStep v-else-if="step === 'project'" ref="current" :api="api" :project="project" @written="projectDone" @settings="openSettings" />
      <BasicsStep v-else-if="step === 'basics'" ref="current" :api="api" :target="basicsTarget" @applied="basicsDone" />
      <AgentStep
        v-else-if="step === 'agent'"
        ref="current"
        :api="mcpApi"
        :root="agentRoot"
        @connected="agentDone"
        @skip="go('done')"
        @settings="openSettings"
      />
      <section v-else class="flex flex-col gap-4" data-testid="setup-done">
        <ul class="flex list-disc flex-col gap-1 ps-5 text-sm" data-testid="setup-summary">
          <li v-for="line in lines" :key="line.key">{{ t(line.key, line.args ?? {}) }}</li>
        </ul>
        <div class="flex flex-col gap-2">
          <h2 class="text-sm font-semibold text-highlighted">{{ t('firstrun.done.where_title') }}</h2>
          <div class="flex flex-wrap gap-2">
            <UButton
              v-for="place in SETTINGS_PLACES"
              :key="place.key"
              color="neutral"
              variant="outline"
              size="sm"
              :label="t(place.labelKey)"
              :data-testid="`setup-where-${place.key}`"
              @click="openSetting(place.key, place.tab)"
            />
          </div>
          <p class="text-xs text-muted">{{ t('firstrun.done.again') }}</p>
        </div>
      </section>
      <footer v-if="step !== 'welcome'" class="sticky bottom-0 flex flex-wrap items-center justify-between gap-2 border-t border-default bg-default py-3">
        <UButton
          color="neutral"
          variant="ghost"
          :icon="SETTINGS_ICONS.previous"
          :label="t('firstrun.back')"
          data-testid="setup-back"
          @click="go(previousStep(step))"
        />
        <UButton v-if="step === 'done'" :label="t('firstrun.finish')" data-testid="setup-finish" @click="close" />
        <UButton
          v-else
          :trailing-icon="SETTINGS_ICONS.next"
          :label="t(nextAction.key, nextAction.args ?? {})"
          :loading="advancing || nextAction.busy === true"
          :disabled="advancing || nextAction.busy === true"
          data-testid="setup-next"
          @click="advance"
        />
      </footer>
    </template>
    <div v-if="enabled === false" class="flex justify-end">
      <UButton :label="t('firstrun.close')" data-testid="setup-close" @click="close" />
    </div>
  </main>
  </div>
</template>
