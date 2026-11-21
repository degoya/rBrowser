<script setup lang="ts">
// The consent check panel (manifest [[panels]], RB-094-05): the notice that this is a technical
// pre-check and no legal advice, always on top; "Check the page" runs the command `check` (the
// findings go to the findings panel), whose post `checked` brings the answer; then the banner, the
// run's caveats and the phases as tabs - before consent, after "Reject all" with the differences,
// the findings. "Write the result" calls the tool `check` with `export`. A run is never started
// by opening the panel.
import { computed, onBeforeUnmount, ref } from 'vue';
import { onPanelEvent } from '../../../../web-common/panel-ui/src/events';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import PanelFrame from '../../../../web-common/panel-ui/src/components/PanelFrame.vue';
import PanelTabs from '../../../../web-common/panel-ui/src/components/PanelTabs.vue';
import PhaseView from './PhaseView.vue';
import { type Answer, bannerText, caveatTexts, diffGroups, findingText, type Section, sortedFindings, statusOf, text } from './model';

const SECTION_ICON: Record<Section, string> = {
  before: 'i-lucide-shield-question',
  after: 'i-lucide-shield-x',
  findings: 'i-lucide-triangle-alert',
};
const SEVERITY_ICON = { error: 'i-lucide-circle-x', warning: 'i-lucide-triangle-alert', info: 'i-lucide-info' } as const;

const panel = usePanel();
const strings = panel.strings;
const tx = (key: string, params?: Record<string, string | number>): string => text(strings.value, key, params);

const answer = ref<Answer | null>(null);
const loading = ref(false);
const failure = ref<string | null>(null);
const exported = ref<string | null>(null);
const reported = ref<number | null>(null);
const tab = ref<Section>('before');

const line = computed(() => {
  if (panel.waiting.value) return { tone: 'busy' as const, text: strings.value['plugins.panel.waiting'] ?? '' };
  return statusOf(strings.value, { failure: failure.value, loading: loading.value, answer: answer.value, exported: exported.value, reported: reported.value });
});
const tabs = computed(() => {
  const current = answer.value;
  if (!current) return [];
  const sections: Section[] = current.afterReject ? ['before', 'after', 'findings'] : ['before', 'findings'];
  return sections.map((name) => ({
    label: tx(`panel.sections.${name}`),
    value: name,
    slot: name,
    icon: SECTION_ICON[name],
    badge: name === 'findings' ? String(current.findings.length) : undefined,
  }));
});
const caveats = computed(() => (answer.value ? caveatTexts(strings.value, answer.value) : []));
const diffs = computed(() => (answer.value ? diffGroups(strings.value, answer.value) : []));
const findings = computed(() => (answer.value ? sortedFindings(answer.value) : []));

function begin(): void {
  loading.value = true;
  failure.value = null;
  exported.value = null;
  reported.value = null;
}

function run(): void {
  begin();
  panel
    .call({ command: 'check' }, {})
    .then(
      (value) => {
        reported.value = (value as { findings?: number } | null)?.findings ?? 0;
      },
      (error: unknown) => {
        failure.value = panel.error(error);
      },
    )
    .finally(() => {
      loading.value = false;
    });
}

function writeExport(): void {
  begin();
  panel
    .call({ tool: 'check' }, { export: true })
    .then(
      (value) => {
        answer.value = value as Answer;
        exported.value = answer.value.exported ?? null;
      },
      (error: unknown) => {
        failure.value = panel.error(error);
      },
    )
    .finally(() => {
      loading.value = false;
    });
}

// The command `check` posts `checked` with the answer, from the panel's button, a shortcut or MCP.
const stopEvents = onPanelEvent((message) => {
  const posted = message as { type?: unknown; answer?: unknown } | null;
  if (posted?.type === 'checked' && posted.answer && typeof posted.answer === 'object') {
    answer.value = posted.answer as Answer;
    if (!answer.value.afterReject && tab.value === 'after') tab.value = 'before';
  }
});
onBeforeUnmount(stopEvents);

panel.onSettings(() => {
  document.title = tx('panels.consent-check');
});
</script>

<template>
  <PanelFrame :title="tx('panels.consent-check')" icon="i-lucide-cookie" :status="line">
    <template #actions>
      <IconAction id="export" icon="i-lucide-file-down" :label="tx('panel.export')" :disabled="!answer || loading" @click="writeExport" />
      <IconAction id="run" icon="i-lucide-play" :label="tx('panel.check')" primary :disabled="loading" @click="run" />
    </template>

    <div class="flex flex-col gap-3">
      <UAlert id="notice" color="neutral" variant="subtle" icon="i-lucide-scale" :title="tx('notice')" :ui="{ title: 'text-sm font-normal' }" />

      <UEmpty v-if="!answer && !loading" id="empty" icon="i-lucide-cookie" :title="tx('panel.empty')" variant="naked" />

      <template v-if="answer">
        <div class="flex flex-col gap-1 text-sm">
          <p class="text-xs break-all text-muted">{{ answer.url }}</p>
          <p id="banner" class="flex items-center gap-1.5">{{ bannerText(strings, answer) }}</p>
          <p id="mode" class="text-xs text-muted">{{ tx(`panel.mode.${answer.mode}`) }}</p>
        </div>

        <UAlert v-if="caveats.length > 0" id="caveats" color="warning" variant="subtle" icon="i-lucide-triangle-alert">
          <template #description>
            <ul class="flex list-disc flex-col gap-1 pl-4 text-sm">
              <li v-for="caveat in caveats" :key="caveat">{{ caveat }}</li>
            </ul>
          </template>
        </UAlert>

        <PanelTabs id="tabs" v-model="tab" :items="tabs" :unmount-on-hide="false" variant="link" size="sm">
          <template #before>
            <section id="before" tabindex="-1" class="pt-2 outline-none">
              <PhaseView :phase="answer.before" />
            </section>
          </template>
          <template #after>
            <section v-if="answer.afterReject" id="after" tabindex="-1" class="flex flex-col gap-4 pt-2 outline-none">
              <p v-if="diffs.length === 0" class="text-sm text-muted">{{ tx('panel.diff.none') }}</p>
              <div v-for="group in diffs" :key="group.id" :data-diff="group.id" class="flex flex-col gap-1">
                <h2 class="text-sm font-semibold text-highlighted">{{ group.title }}</h2>
                <ul class="flex flex-col gap-0.5 text-sm">
                  <li v-for="item in group.items" :key="item" class="break-all">{{ item }}</li>
                </ul>
              </div>
              <PhaseView :phase="answer.afterReject" />
            </section>
          </template>
          <template #findings>
            <section id="findings" tabindex="-1" class="pt-2 outline-none">
              <p v-if="findings.length === 0" class="flex items-center gap-1.5 text-sm text-muted">
                <UIcon name="i-lucide-circle-check" class="size-4 shrink-0 text-success" />
                <span>{{ tx('panel.noFindings') }}</span>
              </p>
              <ul v-else class="flex flex-col gap-2">
                <li v-for="(finding, index) in findings" :key="index" class="flex items-start gap-2 text-sm">
                  <UBadge
                    :color="finding.severity"
                    :icon="SEVERITY_ICON[finding.severity]"
                    variant="subtle"
                    size="sm"
                    class="shrink-0"
                    :label="tx(`panel.severity.${finding.severity}`)"
                  />
                  <span data-finding class="whitespace-normal">{{ findingText(strings, finding) }}</span>
                </li>
              </ul>
            </section>
          </template>
        </PanelTabs>
      </template>
    </div>
  </PanelFrame>
</template>
