<script setup lang="ts">
// An entry's details (RB-080-09, on Nuxt UI since RB-080-84): the resource, the source (file,
// line and column, in full in the tooltip), the request in one line, the detail when it says
// more than the message, and the stack trace coloured by the one highlighter.
import { computed } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import CodeBlock from '../../../../web-common/panel-ui/src/code/CodeBlock.vue';
import { type ReportEntry, requestText, shortSource } from './model';

const props = defineProps<{ entry: ReportEntry }>();
const { t } = usePanel();

const source = computed(() => shortSource(props.entry.source));
const request = computed(() => requestText(props.entry.request));
const detail = computed(() => (props.entry.detail && props.entry.detail !== props.entry.text ? props.entry.detail : ''));
</script>

<template>
  <dl data-details class="mt-1.5 grid cursor-auto grid-cols-[max-content_1fr] gap-x-3 gap-y-1 rounded-md bg-elevated/50 p-2 text-xs">
    <template v-if="entry.url">
      <dt class="text-muted">{{ t('panel.url') }}</dt>
      <dd class="m-0 break-all">{{ entry.url }}</dd>
    </template>
    <template v-if="source">
      <dt class="text-muted">{{ t('panel.source') }}</dt>
      <dd class="m-0 break-all">
        <UTooltip :text="source.full" :content="{ side: 'top', align: 'start' }">
          <span data-source class="font-mono">{{ source.short }}</span>
        </UTooltip>
      </dd>
    </template>
    <template v-if="request">
      <dt class="text-muted">{{ t('panel.request') }}</dt>
      <dd data-request class="m-0 font-mono">{{ request }}</dd>
    </template>
    <template v-if="detail">
      <dt class="text-muted">{{ t('panel.detail') }}</dt>
      <dd class="m-0 break-all whitespace-pre-wrap">{{ detail }}</dd>
    </template>
    <template v-if="entry.stack">
      <dt class="text-muted">{{ t('panel.stack') }}</dt>
      <dd class="m-0 min-w-0">
        <CodeBlock data-stack :text="entry.stack" lang="stack" />
      </dd>
    </template>
  </dl>
</template>
