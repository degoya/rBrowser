<script setup lang="ts">
// The project step's preview (RB-091-05): the views the config opens, then the config itself as
// it will be written, in syntax colours (escaped as text by Vue, tokenised like the settings'
// JSON view).
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import type { SetupJson } from '../ipc/protocol';
import { JSON_TOKEN_CLASSES, tokenizeJsonLines } from '../settings/jsonTokens';
import { proposedViews } from './project';

const props = defineProps<{ config: { [key: string]: SetupJson } }>();
const { t } = useI18n();

const views = computed(() => proposedViews(props.config));
const lines = computed(() => tokenizeJsonLines(JSON.stringify(props.config, null, 2)));

function sizeText(view: (typeof views.value)[number]): string {
  if (view.size === null) return t('firstrun.project.size_unknown');
  return 'device' in view.size
    ? t('firstrun.project.size_device', { device: view.size.device })
    : t('firstrun.project.size', { width: view.size.width, height: view.size.height });
}
</script>

<template>
  <div class="flex flex-col gap-2" data-testid="project-preview-config">
    <h3 class="text-sm font-semibold text-highlighted">{{ t('firstrun.project.views', { count: views.length }) }}</h3>
    <ul class="flex flex-col gap-1 text-sm">
      <li v-for="view in views" :key="view.id" class="flex gap-2" :data-testid="`preview-view-${view.id}`">
        <span class="font-medium">{{ view.label }}</span>
        <span class="text-muted">{{ sizeText(view) }}</span>
      </li>
    </ul>
    <pre
      class="max-h-72 overflow-auto rounded-md bg-elevated p-3 font-mono text-xs text-default"
      tabindex="0"
      role="region"
      :aria-label="t('firstrun.project.config_label')"
      data-testid="preview-json"
    ><template v-for="(line, row) in lines" :key="row"><template v-for="(token, column) in line" :key="column"><span v-if="token.kind !== 'space'" :class="JSON_TOKEN_CLASSES[token.kind]">{{ token.text }}</span><template v-else>{{ token.text }}</template></template>{{ '\n' }}</template></pre>
  </div>
</template>
