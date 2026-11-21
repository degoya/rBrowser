<script setup lang="ts">
// The copy button of a saved bug report's status message (RB-093-04, `copyReport.ts`): an icon
// with its label as tooltip and accessible name, a check while the text is on the clipboard -
// until the message changes. Not a command of the registry: only the clicked document may write
// the clipboard, and the core has none.
import { ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import type { IpcClient } from '../ipc/client';
import type { StatusCopy } from '../ipc/protocol';
import { copyReportMarkdown } from './copyReport';
import { STATUS_ICONS } from './icons';

const props = withDefaults(
  defineProps<{
    client: IpcClient;
    copy: StatusCopy;
    /** The clipboard written; the browser's when absent. */
    clipboard?: Pick<Clipboard, 'writeText'>;
  }>(),
  { clipboard: undefined },
);
const { t } = useI18n();
const copied = ref(false);
const busy = ref(false);

watch(
  () => props.copy.report,
  () => {
    copied.value = false;
  },
);

async function copyIt(): Promise<void> {
  busy.value = true;
  copied.value = await copyReportMarkdown(props.client, props.copy.report, props.clipboard ?? navigator.clipboard);
  busy.value = false;
}
</script>

<template>
  <UButton
    :icon="copied ? STATUS_ICONS.statusCopied : STATUS_ICONS.statusCopy"
    color="neutral"
    variant="ghost"
    size="xs"
    square
    class="shrink-0"
    :disabled="busy"
    :aria-label="copied ? t('report.action.copied') : t(copy.labelKey)"
    :title="copied ? t('report.action.copied') : t(copy.labelKey)"
    data-testid="status-copy"
    @click="copyIt"
  />
</template>
