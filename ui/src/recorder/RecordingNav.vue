<script setup lang="ts">
// The Recorder panel's list of recordings (RB-090-60), newest first, the chosen one marked:
// a click opens one; "Delete" beside each (RB-090-91, `recorder.delete`, `DeleteEntry.vue`)
// asks first while `automation.recorder.confirmDelete` is on and waits while the recording is
// being recorded or plays. Split out of `RecorderPage.vue`, which keeps the state.
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import DeleteEntry from '../flows/DeleteEntry.vue';
import type { RecordingList } from '../ipc/protocol';

const props = defineProps<{
  list: RecordingList;
  chosen: string | null;
  /** "Delete" asks first. */
  confirm: boolean;
  /** The recording being recorded now. */
  recordingNow: string | undefined;
  /** The flow running now: a played recording's name. */
  playing: string | null;
}>();
const emit = defineEmits<{ choose: [name: string]; delete: [name: string] }>();
const { t } = useI18n();
const when = new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' });

/** Why "Delete" of `name` waits, or null. */
function blocked(name: string): string | null {
  if (name === props.recordingNow) {
    return t('recorder.panel.delete_recording', { name });
  }
  return name === props.playing ? t('recorder.panel.delete_playing', { name }) : null;
}
</script>

<template>
  <nav
    class="max-h-48 shrink-0 overflow-auto border-b border-default sm:max-h-none sm:w-56 sm:border-e sm:border-b-0"
    :aria-label="t('recorder.panel.list')"
  >
    <p v-if="list.recordings.length === 0" class="p-3 text-sm text-muted" data-testid="recorder-empty">
      {{ t('recorder.panel.empty') }}
    </p>
    <ul>
      <li
        v-for="entry in list.recordings"
        :key="entry.name"
        class="flex flex-wrap items-center pe-1"
        :data-testid="`recording-entry-${entry.name}`"
      >
        <UButton
          color="neutral"
          :variant="entry.name === chosen ? 'soft' : 'ghost'"
          class="min-w-0 flex-1 justify-start rounded-none text-start"
          :aria-current="entry.name === chosen ? 'true' : undefined"
          :data-testid="`recording-${entry.name}`"
          @click="emit('choose', entry.name)"
        >
          <span class="flex min-w-0 flex-col">
            <span class="truncate font-medium">{{ entry.name }}</span>
            <span class="text-xs text-muted">
              {{ t('recorder.panel.entry', { steps: entry.steps, when: when.format(entry.modifiedMs) }) }}
            </span>
          </span>
        </UButton>
        <DeleteEntry
          :name="entry.name"
          command="recorder.delete"
          :question="t('recorder.panel.delete_question', { name: entry.name })"
          :confirm="confirm"
          :blocked="blocked(entry.name)"
          @delete="emit('delete', entry.name)"
        />
      </li>
    </ul>
  </nav>
</template>
