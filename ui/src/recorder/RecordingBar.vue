<script setup lang="ts">
// The chosen recording's bar in the Recorder panel (RB-090-60, moved out of `RecorderPage.vue`):
// its name, the unsaved and saved notes, the run going on, the pace, Play, Cancel, Revert, Save and
// Export. Below it, in the panel (a panel opens no dialog, design.md 4.4), the two questions that
// keep unsaved edits (RB-092-01 review): another recording chosen - discard the edits or keep
// them -, and the file changed on disk under them - Save waits until Revert reads it.
import { useI18n } from 'vue-i18n';
import UAlert from '@nuxt/ui/components/Alert.vue';
import UButton from '@nuxt/ui/components/Button.vue';
import USelect from '@nuxt/ui/components/Select.vue';
import { RECORDER_ICONS } from './icons';
import type { Pace } from './pace';

defineProps<{
  name: string;
  dirty: boolean;
  notice: string | null;
  /** The flow that runs, or null. */
  runningFlow: string | null;
  paces: { value: Pace; label: string }[];
  playBlocked: string | null;
  playHint: string;
  /** The file changed on disk under unsaved edits. */
  stale: boolean;
  /** Another recording the person chose while edits are unsaved. */
  pendingChoice: string | null;
}>();
const pace = defineModel<Pace>('pace', { required: true });
const emit = defineEmits<{
  play: [];
  cancelRun: [];
  revert: [];
  save: [];
  export: [];
  discard: [];
  keep: [];
}>();
const { t } = useI18n();
</script>

<template>
  <div class="flex flex-col">
    <div class="flex flex-wrap items-center gap-2 border-b border-default px-3 py-2">
      <span class="truncate text-sm font-medium">{{ name }}</span>
      <span v-if="dirty" class="text-xs text-warning" data-testid="recorder-dirty">{{ t('recorder.panel.unsaved') }}</span>
      <span v-if="notice !== null" class="text-xs text-success" role="status" data-testid="recorder-notice">{{ notice }}</span>
      <span class="text-xs text-muted" role="status" aria-live="polite" data-testid="recorder-running">{{
        runningFlow === null ? '' : t('recorder.panel.running', { flow: runningFlow })
      }}</span>
      <span class="ms-auto flex flex-wrap items-center gap-1">
        <USelect v-model="pace" :items="paces" size="sm" class="w-40" :aria-label="t('recorder.panel.pace.label')" data-testid="recorder-pace" />
        <UButton
          v-if="runningFlow !== null"
          :icon="RECORDER_ICONS.flowCancel"
          size="sm"
          color="error"
          variant="soft"
          :label="t('recorder.panel.cancel')"
          data-testid="recorder-cancel"
          @click="emit('cancelRun')"
        />
        <UButton
          :icon="RECORDER_ICONS.flowRun"
          size="sm"
          :label="t('recorder.panel.play')"
          :disabled="playBlocked !== null"
          :title="playBlocked ?? playHint"
          data-testid="recorder-play"
          @click="emit('play')"
        />
        <UButton
          :icon="RECORDER_ICONS.recorderRevert"
          size="sm"
          color="neutral"
          variant="ghost"
          :label="t('recorder.panel.revert')"
          :disabled="!dirty && !stale"
          data-testid="recorder-revert"
          @click="emit('revert')"
        />
        <UButton
          :icon="RECORDER_ICONS.recorderSave"
          size="sm"
          :label="t('recorder.panel.save')"
          :disabled="!dirty || stale"
          :title="stale ? t('recorder.editor.stale_save') : undefined"
          data-testid="recorder-save"
          @click="emit('save')"
        />
        <UButton
          :icon="RECORDER_ICONS.recorderExport"
          size="sm"
          color="neutral"
          variant="outline"
          :label="t('recorder.panel.export')"
          :disabled="dirty"
          :title="dirty ? t('recorder.panel.export_unsaved') : undefined"
          data-testid="recorder-export"
          @click="emit('export')"
        />
      </span>
    </div>
    <UAlert
      v-if="pendingChoice !== null"
      color="warning"
      variant="subtle"
      orientation="horizontal"
      role="alert"
      class="rounded-none"
      :title="t('recorder.editor.discard_question', { name, other: pendingChoice })"
      data-testid="recorder-discard"
    >
      <template #actions>
        <UButton size="sm" color="error" :label="t('recorder.editor.discard')" data-testid="recorder-discard-confirm" @click="emit('discard')" />
        <UButton size="sm" color="neutral" variant="ghost" :label="t('recorder.editor.keep')" data-testid="recorder-discard-keep" @click="emit('keep')" />
      </template>
    </UAlert>
    <UAlert
      v-if="stale"
      color="warning"
      variant="subtle"
      orientation="horizontal"
      role="alert"
      class="rounded-none"
      :title="t('recorder.editor.stale', { name })"
      data-testid="recorder-stale"
    >
      <template #actions>
        <UButton
          size="sm"
          color="warning"
          :icon="RECORDER_ICONS.recorderRevert"
          :label="t('recorder.editor.stale_revert')"
          data-testid="recorder-stale-revert"
          @click="emit('revert')"
        />
      </template>
    </UAlert>
  </div>
</template>
