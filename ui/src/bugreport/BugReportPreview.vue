<script setup lang="ts">
// The bug report preview (RB-093-04), the route `bug-report` of the pages window: the report a
// command gathered, every text masked, before anything is saved - `report.md` as it will be
// written (plain text, never rendered: a page's texts stay text), how many screenshots were
// taken with the password fields blacked out, the folder, and whether the screenshot editor
// opens afterwards (`report.annotate`). Save writes `report.md` and `report.zip`; Cancel (or
// Escape, or closing the window) drops the report with its screenshots. The core opens the page
// while `report.preview` is on and closes the window once the word is in.
import { computed, onMounted } from 'vue';
import { useI18n } from 'vue-i18n';
import UAlert from '@nuxt/ui/components/Alert.vue';
import UButton from '@nuxt/ui/components/Button.vue';
import type { IpcClient } from '../ipc/client';
import { createPreview } from './preview';

defineOptions({ inheritAttrs: false });
const props = defineProps<{ client: IpcClient }>();
const { t } = useI18n();
const preview = createPreview(props.client);

const state = computed(() => preview.state.value);
const shown = computed(() =>
  state.value.phase === 'open' || state.value.phase === 'sending' ? state.value.preview : null,
);
const busy = computed(() => state.value.phase === 'sending');
const refused = computed(() => state.value.phase === 'open' && state.value.failed === true);

function cancel(): void {
  if (shown.value !== null && !busy.value) void preview.decide(false);
}

onMounted(() => void preview.load());
</script>

<template>
  <main
    class="flex h-screen flex-col gap-3 bg-default p-4 text-sm text-default"
    :aria-label="t('report.preview.title')"
    data-testid="bug-report-preview"
    @keydown.escape.prevent="cancel"
  >
    <h1 class="text-base font-semibold text-highlighted">{{ t('report.preview.title') }}</h1>
    <p v-if="state.phase === 'loading'" class="text-muted" data-testid="bug-report-loading">
      {{ t('report.preview.loading') }}
    </p>
    <p v-else-if="state.phase === 'none'" class="text-muted" data-testid="bug-report-none">
      {{ t('report.preview.none') }}
    </p>
    <UAlert
      v-else-if="state.phase === 'failed'"
      color="error"
      variant="subtle"
      :title="t('report.preview.failed')"
      data-testid="bug-report-failed"
    />
    <p v-else-if="state.phase === 'decided'" class="text-muted" data-testid="bug-report-decided">
      {{ state.saved ? t('report.preview.saved') : t('report.preview.cancelled') }}
    </p>
    <template v-if="shown !== null">
      <p data-testid="bug-report-intro">{{ t('report.preview.intro', { count: shown.images }) }}</p>
      <p class="truncate text-muted" :title="shown.folder" data-testid="bug-report-folder">
        {{ t('report.preview.folder', { folder: shown.folder }) }}
      </p>
      <p v-if="shown.annotate" class="text-muted" data-testid="bug-report-annotate">
        {{ t('report.preview.annotate') }}
      </p>
      <pre
        class="min-h-0 flex-1 overflow-auto whitespace-pre-wrap break-words rounded-md bg-elevated p-3 font-mono text-xs text-default"
        tabindex="0"
        role="region"
        :aria-label="t('report.preview.markdown_label')"
        data-testid="bug-report-markdown"
      >{{ shown.markdown }}</pre>
      <UAlert
        v-if="refused"
        color="error"
        variant="subtle"
        :title="t('report.preview.refused')"
        data-testid="bug-report-refused"
      />
      <div class="flex justify-end gap-2">
        <UButton
          color="neutral"
          variant="ghost"
          :disabled="busy"
          :label="t('report.preview.cancel')"
          data-testid="bug-report-cancel"
          @click="cancel"
        />
        <UButton
          :loading="busy"
          :disabled="busy"
          :label="t('report.preview.save')"
          data-testid="bug-report-save"
          @click="preview.decide(true)"
        />
      </div>
    </template>
  </main>
</template>
