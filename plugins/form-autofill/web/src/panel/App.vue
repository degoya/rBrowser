<script setup lang="ts">
// The form autofill panel (RB-070-11) on Nuxt UI (RB-080-86). "Fill valid" and "Fill invalid"
// call the tool `fill` (every view) with the options - the form, the invalid values, the profile
// and the seed - which fold away under "Options"; a fill from a shortcut or the palette reaches
// the panel as the event `filled`. The result is a row per field, not a column per view: the
// value once, the views it differs in counted and opened on request, the fields skipped
// everywhere folded apart, a section per form; the views not filled are said once in the status
// line with the fields and views filled and the seed. "Take form as preset" (RB-090-70) reads the
// active view's form through the tool `capture` - or the view the command `capture` names in its
// post, or in its request when the command opened this panel - and opens the preset dialog; the
// presets stored join the profile choice (RB-090-71: always in sight, the settings' profiles and the
// presets in groups of their own, a preset renamed or deleted there; a preset just saved is chosen).
import { computed, onBeforeUnmount, ref } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { onPanelEvent } from '../../../../web-common/panel-ui/src/events';
import type { StatusLine } from '../../../../web-common/panel-ui/src/status';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import PanelFrame from '../../../../web-common/panel-ui/src/components/PanelFrame.vue';
import FillOptions from './FillOptions.vue';
import FillResult from './FillResult.vue';
import ProfileChoice from './ProfileChoice.vue';
import CaptureDialog from './CaptureDialog.vue';
import { type Capture, isCapture, presetNames, profileChoices } from './capture';
import { type AutofillForm, type AutofillResult, type FillForm, fillArgs, formsOf, outcome, profiles, statusLine, STRATEGIES } from './model';

const panel = usePanel();
const t = panel.t;

const result = ref<AutofillResult | null>(null);
const forms = ref<AutofillForm[]>([]);
const busy = ref(false);
const failure = ref<string | null>(null);
const tooLarge = ref<number | null>(null);
const open = ref(false);
const options = ref<FillForm>({ selector: '', strategy: 'mixed', profile: '', seed: null });
let profileTouched = false;
const presets = ref<string[]>([]);
const capture = ref<Capture | null>(null);
const dialogOpen = ref(false);
const reading = ref(false);
const notice = ref<string | null>(null);

const settingsProfiles = computed(() => profiles(panel.settings.value));
const profileNames = computed(() => profileChoices(settingsProfiles.value, presets.value).map((choice) => choice.name));
const profile = computed({
  get: () => options.value.profile,
  set: (value: string) => update({ ...options.value, profile: value }),
});
/** What Fill valid fills with, said in its tooltip (RB-090-72). */
const validTip = computed(() =>
  options.value.profile ? t('panel.fillValidWith', { name: options.value.profile }) : t('panel.fillValidNone'),
);
const seen = computed(() => {
  void panel.strings.value;
  return result.value ? outcome(result.value, t) : null;
});

/** A failure code as a sentence, its blanks closed (a code comes without its parameters). */
const say = (failed: unknown): string => panel.error(failed).replace(/\s{2,}/g, ' ');

const line = computed<StatusLine | null>(() => {
  void panel.strings.value;
  if (busy.value) return { tone: 'busy', text: panel.waiting.value ? panel.strings.value['plugins.panel.waiting'] ?? t('panel.running') : t('panel.running') };
  if (reading.value) return { tone: 'busy', text: t('panel.capture.reading') };
  if (failure.value) return { tone: 'error', text: failure.value };
  if (notice.value) return { tone: 'ok', text: notice.value };
  if (tooLarge.value !== null) return { tone: 'info', text: t('panel.tooLarge', { seed: tooLarge.value }) };
  if (result.value && seen.value) return statusLine(t, (code) => say(new Error(code)), result.value, seen.value);
  return null;
});

function show(answer: AutofillResult): void {
  result.value = answer;
  tooLarge.value = null;
  failure.value = null;
  const found = formsOf(answer);
  if (found.length > 0) forms.value = found;
}

async function fill(mode: 'valid' | 'invalid'): Promise<void> {
  if (busy.value) return;
  busy.value = true;
  failure.value = null;
  notice.value = null;
  try {
    show((await panel.call({ tool: 'fill' }, fillArgs(mode, options.value))) as AutofillResult);
  } catch (error) {
    failure.value = t('panel.error', { text: say(error) });
  } finally {
    busy.value = false;
  }
}

/** The profile the settings name, once it is among the choices (a preset arrives later). */
function followActive(): void {
  if (profileTouched) return;
  const active = String(panel.settings.value['form-autofill.activeProfile'] ?? '');
  options.value = { ...options.value, profile: profileNames.value.includes(active) ? active : '' };
}

async function loadPresets(): Promise<void> {
  try {
    presets.value = presetNames(await panel.call({ tool: 'presets' }));
    followActive();
  } catch {
    // No list, no presets in the choice: the fill names its failure itself.
  }
}

/**
 * Reads a form (`args`: the view, the request of the command) and opens the dialog. `quiet`: the
 * look for a pending request when the panel starts - no busy line, and nothing waits for it.
 */
async function takeForm(args: Record<string, unknown>, quiet = false): Promise<void> {
  if (!quiet) {
    if (reading.value) return;
    reading.value = true;
    failure.value = null;
    notice.value = null;
  }
  try {
    const answer = await panel.call({ tool: 'capture' }, args);
    if (!isCapture(answer)) return;
    capture.value = answer;
    presets.value = answer.presets;
    dialogOpen.value = true;
  } catch (error) {
    failure.value = t('panel.capture.readFailed', { text: say(error) });
  } finally {
    if (!quiet) reading.value = false;
  }
}

function saved(preset: { name: string; fields: number }): void {
  if (!presets.value.includes(preset.name)) presets.value = [...presets.value, preset.name];
  // The preset just taken is the one Fill valid uses next (RB-090-71).
  profile.value = preset.name;
  failure.value = null;
  notice.value = t('panel.capture.saved', { name: preset.name, count: preset.fields });
}

function renamed({ from, to }: { from: string; to: string }): void {
  presets.value = presets.value.map((name) => (name === from ? to : name));
  if (options.value.profile === from) options.value = { ...options.value, profile: to };
  failure.value = null;
  notice.value = t('panel.choice.renamed', { from, to });
}

function deleted(name: string): void {
  presets.value = presets.value.filter((entry) => entry !== name);
  if (options.value.profile === name) options.value = { ...options.value, profile: '' };
  failure.value = null;
  notice.value = t('panel.choice.deleted', { name });
}

function update(next: FillForm): void {
  if (next.profile !== options.value.profile) profileTouched = true;
  options.value = next;
}

// The options start from the settings; the profile follows them until it is chosen here.
panel.onSettings((first) => {
  const settings = panel.settings.value;
  const strategy = String(settings['form-autofill.invalidStrategy'] ?? 'mixed');
  const active = String(settings['form-autofill.activeProfile'] ?? '');
  const next = { ...options.value };
  if (first) next.strategy = (STRATEGIES as readonly string[]).includes(strategy) ? strategy : 'mixed';
  if (!profileTouched) next.profile = active;
  if (!profileNames.value.includes(next.profile)) next.profile = '';
  options.value = next;
  if (first) {
    void loadPresets();
    // The command `capture` opened this panel: its post came before the panel listened.
    void takeForm({ pending: true }, true);
  }
});

// A fill from a shortcut or the palette, posted by the plugin (panel-ui hears only the panels
// window).
const stopEvents = onPanelEvent((message) => {
  const posted = message as { type?: unknown; result?: AutofillResult; seed?: number; view?: unknown } | null;
  if (!posted || typeof posted !== 'object') return;
  if (posted.type === 'capture' && typeof posted.view === 'number') {
    void takeForm({ target: posted.view, pending: true });
    return;
  }
  if (posted.type === 'presets') {
    presets.value = presetNames(posted);
    return;
  }
  if (posted.type !== 'filled') return;
  if (posted.result) show(posted.result);
  else tooLarge.value = Number(posted.seed ?? 0);
});
onBeforeUnmount(stopEvents);
</script>

<template>
  <PanelFrame :title="t('panel.title')" icon="i-lucide-text-cursor-input" :status="line">
    <template #actions>
      <UFieldGroup size="sm">
        <UTooltip :text="validTip">
          <UButton
            id="fill-valid"
            data-primary
            icon="i-lucide-circle-check"
            :label="t('panel.fillValid')"
            :loading="busy"
            :disabled="busy"
            @click="fill('valid')"
          />
        </UTooltip>
        <UTooltip :text="t('panel.fillInvalidTip')">
          <UButton
            id="fill-invalid"
            color="primary"
            variant="outline"
            icon="i-lucide-circle-x"
            :label="t('panel.fillInvalid')"
            :disabled="busy"
            @click="fill('invalid')"
          />
        </UTooltip>
      </UFieldGroup>
      <IconAction
        id="take-form"
        icon="i-lucide-clipboard-copy"
        :label="t('panel.capture.action')"
        :disabled="reading"
        @click="takeForm({})"
      />
      <IconAction
        id="options"
        icon="i-lucide-sliders-horizontal"
        :label="t('panel.options')"
        :aria-expanded="open ? 'true' : 'false'"
        aria-controls="options-panel"
        @click="open = !open"
      />
    </template>

    <div class="flex flex-col gap-3">
      <CaptureDialog v-model:open="dialogOpen" :capture="capture" @saved="saved" />
      <ProfileChoice
        v-model="profile"
        :profiles="settingsProfiles"
        :presets="presets"
        :filling="busy"
        @renamed="renamed"
        @deleted="deleted"
        @fill="fill('valid')"
      />
      <UCollapsible v-model:open="open" :unmount-on-hide="false">
        <template #content>
          <FillOptions
            id="options-panel"
            :model-value="options"
            :forms="forms"
            @update:model-value="update"
          />
        </template>
      </UCollapsible>

      <UEmpty v-if="!result" id="empty" icon="i-lucide-text-cursor-input" :title="t('panel.empty')" variant="naked" />
      <FillResult v-else-if="seen" :result="result" :seen="seen" />
    </div>
  </PanelFrame>
</template>
