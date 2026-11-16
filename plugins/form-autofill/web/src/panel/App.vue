<script setup lang="ts">
// The form autofill panel (RB-070-11) on Nuxt UI (RB-080-86). "Fill valid" and "Fill invalid"
// call the tool `fill` (every view) with the options - the form, the invalid values, the profile
// and the seed - which fold away under "Options"; a fill from a shortcut or the palette reaches
// the panel as the event `filled`. The result is a row per field, not a column per view: the
// value once, the views it differs in counted and opened on request, the fields skipped
// everywhere folded apart, a section per form; the views not filled are said once in the status
// line with the fields and views filled and the seed.
import { computed, onBeforeUnmount, ref } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { onPanelEvent } from '../../../../web-common/panel-ui/src/events';
import type { StatusLine } from '../../../../web-common/panel-ui/src/status';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import PanelFrame from '../../../../web-common/panel-ui/src/components/PanelFrame.vue';
import FillOptions from './FillOptions.vue';
import FillResult from './FillResult.vue';
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

const profileNames = computed(() => profiles(panel.settings.value));
const seen = computed(() => {
  void panel.strings.value;
  return result.value ? outcome(result.value, t) : null;
});

/** A failure code as a sentence, its blanks closed (a code comes without its parameters). */
const say = (failed: unknown): string => panel.error(failed).replace(/\s{2,}/g, ' ');

const line = computed<StatusLine | null>(() => {
  void panel.strings.value;
  if (busy.value) return { tone: 'busy', text: panel.waiting.value ? panel.strings.value['plugins.panel.waiting'] ?? t('panel.running') : t('panel.running') };
  if (failure.value) return { tone: 'error', text: failure.value };
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
  try {
    show((await panel.call({ tool: 'fill' }, fillArgs(mode, options.value))) as AutofillResult);
  } catch (error) {
    failure.value = t('panel.error', { text: say(error) });
  } finally {
    busy.value = false;
  }
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
});

// A fill from a shortcut or the palette, posted by the plugin (panel-ui hears only the panels
// window).
const stopEvents = onPanelEvent((message) => {
  const posted = message as { type?: unknown; result?: AutofillResult; seed?: number } | null;
  if (!posted || typeof posted !== 'object' || posted.type !== 'filled') return;
  if (posted.result) show(posted.result);
  else tooLarge.value = Number(posted.seed ?? 0);
});
onBeforeUnmount(stopEvents);
</script>

<template>
  <PanelFrame :title="t('panel.title')" icon="i-lucide-text-cursor-input" :status="line">
    <template #actions>
      <UFieldGroup size="sm">
        <UButton
          id="fill-valid"
          data-primary
          icon="i-lucide-circle-check"
          :label="t('panel.fillValid')"
          :loading="busy"
          :disabled="busy"
          @click="fill('valid')"
        />
        <UButton
          id="fill-invalid"
          color="primary"
          variant="outline"
          icon="i-lucide-circle-x"
          :label="t('panel.fillInvalid')"
          :disabled="busy"
          @click="fill('invalid')"
        />
      </UFieldGroup>
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
      <UCollapsible v-model:open="open" :unmount-on-hide="false">
        <template #content>
          <FillOptions
            id="options-panel"
            :model-value="options"
            :forms="forms"
            :profiles="profileNames"
            @update:model-value="update"
          />
        </template>
      </UCollapsible>

      <UEmpty v-if="!result" id="empty" icon="i-lucide-text-cursor-input" :title="t('panel.empty')" variant="naked" />
      <FillResult v-else-if="seen" :result="result" :seen="seen" />
    </div>
  </PanelFrame>
</template>
