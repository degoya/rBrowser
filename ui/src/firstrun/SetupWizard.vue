<script setup lang="ts">
// The setup wizard (RB-091-05), the route `setup` of the pages window: welcome, plugins,
// project, basics, done - optional, every step skippable. "Skip setup" on every page and
// "Finish" close the window; the browser is fully usable without it. The core opens it by
// itself once at the very first start, the command `app.setup_wizard` again; the hint "Set up
// project" opens it at the project step (`#/setup/project`). With `ui.firstRunWizard` off it
// says so and offers only Close. Every control is Nuxt UI; each step is a component of its own.
import { computed, onMounted, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import UAlert from '@nuxt/ui/components/Alert.vue';
import UButton from '@nuxt/ui/components/Button.vue';
import UStepper from '@nuxt/ui/components/Stepper.vue';
import type { IpcClient } from '../ipc/client';
import type { SetupAnswer, SetupProject } from '../ipc/protocol';
import { SETTINGS_ICONS } from '../settings/icons';
import { createSetupApi } from './api';
import BasicsStep from './BasicsStep.vue';
import PluginsStep from './PluginsStep.vue';
import ProjectStep from './ProjectStep.vue';
import { nextStep, previousStep, settingsHash, startStep, STEPS, type StepId } from './steps';
import { type BasicsChoice, EMPTY_SUMMARY, SETTINGS_PLACES, type SetupSummary, summaryLines } from './summary';
import type { Available, PluginsDone } from './plugins';

const props = defineProps<{ client: IpcClient; routeKey?: string | null }>();
const { t } = useI18n();
const api = createSetupApi(props.client);

const step = ref<StepId>(startStep(props.routeKey ?? null));
const enabled = ref<boolean | null>(null);
const project = ref<SetupProject | undefined>(undefined);
const summary = ref<SetupSummary>({ ...EMPTY_SUMMARY });

const STEP_ICONS: Record<StepId, string> = {
  welcome: SETTINGS_ICONS.welcome,
  plugins: SETTINGS_ICONS.plugins,
  project: SETTINGS_ICONS.project,
  basics: SETTINGS_ICONS.basics,
  done: SETTINGS_ICONS.done,
};
const items = computed(() => STEPS.map((value) => ({ value, title: t(`firstrun.step.${value}`), icon: STEP_ICONS[value] })));
/** Where the basics' layout and sync go: the project written here, else the open one with a config. */
const basicsTarget = computed(() => {
  if (summary.value.project !== null) return summary.value.project.root;
  return project.value?.hasConfig === true ? project.value.root : null;
});
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

/** The plugin step was skipped - "Skip setup", "Just start", or past it without Apply: while a
 * fresh install's choice is pending (RB-091-04), the core installs its recommended set, once. A
 * choice made before is left as it is. */
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

watch(step, (next, previous) => {
  if (previous === 'plugins' && STEPS.indexOf(next) > STEPS.indexOf('plugins')) void skipPlugins();
});

async function close(): Promise<void> {
  if (enabled.value !== false) await skipPlugins();
  await api.close().catch(() => undefined);
}

function openSetting(key: string, tab: string | null): void {
  window.location.hash = settingsHash(key, tab);
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
    />
    <template v-else>
      <UStepper v-model="step" :items="items" :linear="false" size="sm" data-testid="setup-stepper" />
      <section v-if="step === 'welcome'" class="flex flex-col gap-4" data-testid="setup-welcome">
        <p class="text-base">{{ t('firstrun.welcome.text') }}</p>
        <p class="text-sm text-muted">{{ t('firstrun.welcome.optional') }}</p>
        <div class="flex flex-wrap gap-2">
          <UButton :icon="SETTINGS_ICONS.wizard" :label="t('firstrun.welcome.setup')" data-testid="setup-start" @click="go('plugins')" />
          <UButton color="neutral" variant="outline" :label="t('firstrun.welcome.just_start')" data-testid="setup-just-start" @click="close" />
        </div>
      </section>
      <PluginsStep v-else-if="step === 'plugins'" :api="api" @installed="pluginsDone" @loaded="pluginsLoaded" />
      <ProjectStep v-else-if="step === 'project'" :api="api" :project="project" @written="projectDone" />
      <BasicsStep v-else-if="step === 'basics'" :api="api" :target="basicsTarget" @applied="basicsDone" />
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
          color="neutral"
          variant="outline"
          :trailing-icon="SETTINGS_ICONS.next"
          :label="t('firstrun.next')"
          data-testid="setup-next"
          @click="go(nextStep(step))"
        />
      </footer>
    </template>
    <div v-if="enabled === false" class="flex justify-end">
      <UButton :label="t('firstrun.close')" data-testid="setup-close" @click="close" />
    </div>
  </main>
  </div>
</template>
