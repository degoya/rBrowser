<script setup lang="ts">
// The rulers-guides panel (RB-060-05), on Nuxt UI since RB-080-86: what is shown (a switch per
// layer), the guides with exact values (GuideList), the column setup per breakpoint
// (ColumnSetup), the baseline grid (BaselineSetup) and the colours (ColorSetup, RB-090-58) - all
// by keyboard. Every change is one call of the tool `set` (`rulers_set`), whose answer - and the
// state the component posts after a change in any view - it draws. Nothing shows and nothing
// starts before its texts arrived (RB-080-59): a raw key never shows. A failure is a sentence of
// the plugin's (RB-080-66), never a code.
import { computed, onBeforeUnmount, ref } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { onPanelEvent } from '../../../../web-common/panel-ui/src/events';
import type { CallError } from '../../../../web-common/panel-ui/src/panel';
import type { StatusLine, Tone } from '../../../../web-common/panel-ui/src/status';
import PanelFrame from '../../../../web-common/panel-ui/src/components/PanelFrame.vue';
import BaselineSetup from './BaselineSetup.vue';
import ColorSetup from './ColorSetup.vue';
import ColumnSetup from './ColumnSetup.vue';
import GuideList from './GuideList.vue';
import { failure, LAYERS, type RulersState, type SetAnswer, type Template } from './model';

const panel = usePanel();
const t = panel.t;

const state = ref<RulersState | null>(null);
const breakpoints = ref<string[]>([]);
/** The templates' values, as the component answers them: the empty fields' placeholders. */
const templates = ref<Record<string, Template>>({});
const message = ref<StatusLine | null>(null);
/** The host's code of a plugin paused after repeated failures. */
const SUSPENDED = 'plugins.error.suspended';

const say = (text: string, tone: Tone = 'info'): void => {
  message.value = { tone, text };
};
function failed(error: unknown): void {
  const code = error instanceof Error ? error.message : String(error);
  say(t('panel.failed', { error: failure(t, panel.strings.value, code, (error as CallError).params ?? {}) }), 'error');
  // "Resume them in Settings > Plugins" links there (RB-094-21).
  if (code === SUSPENDED && message.value) message.value = { ...message.value, setting: { key: 'plugins', label: t('panel.setting_link') } };
}

/** The status line: the host's waiting, else what the last change said. */
const line = computed<StatusLine | null>(() => {
  if (panel.waiting.value) return { tone: 'busy', text: panel.strings.value['plugins.panel.waiting'] ?? '' };
  return message.value;
});

function adopt(answer: unknown): void {
  const now = answer as SetAnswer | null;
  if (!now || typeof now !== 'object' || !now.state) return;
  state.value = now.state;
  if (Array.isArray(now.breakpoints)) breakpoints.value = now.breakpoints.map(String);
  if (now.templates && typeof now.templates === 'object') templates.value = now.templates;
}

function change(args: Record<string, unknown>): void {
  panel
    .call({ tool: 'set' }, args)
    .then((answer) => {
      adopt(answer);
      const saved = (answer as SetAnswer | null)?.saved;
      if (typeof saved === 'boolean') say(t(saved ? 'panel.saved' : 'panel.not_saved'), saved ? 'ok' : 'info');
    })
    .catch(failed);
}

// The plugin's posts; the host's own word (`waiting`) is panel.ts's.
const stopEvents = onPanelEvent((message) => {
  if (!message || typeof message !== 'object' || 'host' in message) return;
  adopt(message);
});
onBeforeUnmount(stopEvents);
panel.onSettings((first) => {
  if (first) panel.call({ tool: 'set' }, {}).then(adopt, failed);
});
</script>

<template>
  <PanelFrame v-if="panel.ready.value" :title="t('panels.guides')" icon="i-lucide-ruler" :status="line">
    <div v-if="!state" id="state" role="status">
      <UEmpty icon="i-lucide-loader-circle" variant="naked" size="sm" :ui="{ avatar: 'animate-spin' }" />
    </div>
    <div v-else class="flex max-w-2xl flex-col gap-6">
      <fieldset id="show" class="flex flex-col gap-2">
        <legend class="mb-2 text-sm font-semibold text-highlighted">{{ t('panel.show') }}</legend>
        <USwitch
          v-for="layer in LAYERS"
          :id="`show-${layer}`"
          :key="layer"
          :model-value="state.visible[layer]"
          :label="t(`panel.show_${layer}`)"
          :data-visible="layer"
          @update:model-value="(on: boolean) => change({ visible: { [layer]: on } })"
        />
      </fieldset>
      <GuideList :guides="state.guides" @change="change" />
      <ColumnSetup :state="state" :breakpoints="breakpoints" :templates="templates" @change="change" />
      <BaselineSetup :state="state" @change="change" />
      <ColorSetup :state="state" @change="change" />
      <section class="flex flex-col items-start gap-1 border-t border-default pt-4">
        <UButton id="reset" icon="i-lucide-rotate-ccw" color="neutral" variant="outline" size="sm" :label="t('panel.reset')" aria-describedby="reset-hint" @click="change({ reset: true })" />
        <p id="reset-hint" class="text-xs text-muted">{{ t('panel.reset_hint') }}</p>
      </section>
    </div>
  </PanelFrame>
</template>
