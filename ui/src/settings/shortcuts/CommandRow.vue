<script setup lang="ts">
// One command in the shortcut editor: its bindings, record a new one by pressing it, remove one,
// reset the command. A recorded binding is checked by the core first: a reserved combination is
// refused with the reason, one another command holds opens the conflict dialog (RB-060-11). A
// plugin default the registry could not bind is marked. The pages window has no rBrowser
// shortcuts, so every key reaches the recorder. The id is there on request (RB-080-70): the
// title's tooltip, under it with `settings.showKeys`; the search finds it either way. When the
// recorder, the conflict dialog or a removed binding's button goes away, or Reset disables
// itself, the focus goes to the row's Record button, never to the body (RB-090-23).
import { computed, nextTick, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import UKbd from '@nuxt/ui/components/Kbd.vue';
import type { CommandInfo } from '../../ipc/protocol';
import { SETTINGS_ICONS } from '../icons';
import ConflictPrompt from './ConflictPrompt.vue';
import { resolveConflict, type ConflictChoice } from './conflict';
import { droppedDefaults } from './filter';
import { Recorder } from './record';
import type { ShortcutsStore } from './store';

const props = defineProps<{
  command: CommandInfo;
  store: ShortcutsStore;
  /** Print the id under the title (`settings.showKeys`, RB-080-70); off it is the tooltip. */
  showKeys?: boolean;
}>();
const { t } = useI18n();

const recording = ref(false);
const typed = ref('');
const refusal = ref<string | null>(null);
const conflict = ref<{ binding: string; holder: CommandInfo } | null>(null);
const capture = ref<HTMLElement | null>(null);
const recordButton = ref<{ $el: HTMLElement } | null>(null);
const recorder = new Recorder();
const title = computed(() => t(props.command.titleKey));
const dropped = computed(() => droppedDefaults(props.command));

async function startRecording(): Promise<void> {
  recorder.reset();
  typed.value = '';
  refusal.value = null;
  conflict.value = null;
  recording.value = true;
  await nextTick();
  capture.value?.focus();
}

function stopRecording(): void {
  recording.value = false;
  typed.value = '';
}

/** The focus to the row's Record button, once it is drawn again. */
async function focusRecord(): Promise<void> {
  await nextTick();
  recordButton.value?.$el.focus();
}

async function onKey(event: KeyboardEvent): Promise<void> {
  event.preventDefault();
  event.stopPropagation();
  const step = recorder.feed(event);
  typed.value = recorder.text();
  if (step === 'cancel') {
    stopRecording();
    await focusRecord();
  } else if (step === 'done') {
    const binding = recorder.text();
    stopRecording();
    await focusRecord();
    await record(binding);
  }
}

async function record(binding: string): Promise<void> {
  try {
    const check = await props.store.check(props.command.id, binding);
    if (check.ok) {
      await props.store.add(props.command, binding);
      return;
    }
    const holder = props.store.commands.value.find((c) => c.id === check.conflict);
    if (holder !== undefined) {
      conflict.value = { binding, holder };
      return;
    }
    const reason = check.reasonKey === undefined ? '' : t(check.reasonKey);
    refusal.value = t('settings.shortcuts.refused', { binding, reason });
  } catch {
    refusal.value = t('settings.shortcuts.failed', { binding });
  }
}

async function resolve(choice: ConflictChoice): Promise<void> {
  const open = conflict.value;
  conflict.value = null;
  await focusRecord();
  if (open !== null) {
    await props.store.apply(resolveConflict(choice, props.command, open.holder, open.binding));
  }
}

async function remove(binding: string): Promise<void> {
  await props.store.remove(props.command, binding);
  await focusRecord();
}

async function reset(): Promise<void> {
  await props.store.reset(props.command.id);
  await focusRecord();
}
</script>

<template>
  <div class="flex flex-wrap items-center gap-2 rounded-md border border-default px-3 py-2" :data-command="command.id">
    <div class="min-w-48 flex-1">
      <div class="text-sm text-default" :title="command.id" data-testid="command-title">{{ title }}</div>
      <div v-if="showKeys" class="font-mono text-xs text-muted" data-testid="command-id">{{ command.id }}</div>
    </div>
    <div class="flex flex-wrap items-center gap-1">
      <span v-for="binding in command.bindings" :key="binding" class="flex items-center gap-0.5">
        <UKbd :value="binding" size="md" data-testid="binding" />
        <UButton
          :icon="SETTINGS_ICONS.close"
          color="neutral"
          variant="ghost"
          size="xs"
          square
          :aria-label="t('settings.shortcuts.remove', { binding })"
          :title="t('settings.shortcuts.remove', { binding })"
          @click="remove(binding)"
        />
      </span>
      <span v-if="command.bindings.length === 0" class="text-xs text-muted">{{ t('settings.shortcuts.unbound') }}</span>
    </div>
    <div
      v-if="recording"
      ref="capture"
      tabindex="0"
      role="textbox"
      aria-live="polite"
      :aria-label="t('settings.shortcuts.recording')"
      class="min-w-48 rounded-md bg-elevated px-2 py-1 font-mono text-sm ring-2 ring-primary focus-visible:outline-none"
      data-testid="recorder"
      @keydown="onKey"
      @blur="stopRecording"
    >
      {{ typed || t('settings.shortcuts.recording') }}
    </div>
    <UButton
      v-else
      ref="recordButton"
      :icon="SETTINGS_ICONS.record"
      color="neutral"
      variant="soft"
      size="xs"
      :label="t('settings.shortcuts.record')"
      :aria-label="t('settings.shortcuts.record_for', { command: title })"
      data-testid="record"
      @click="startRecording"
    />
    <UButton
      :icon="SETTINGS_ICONS.reset"
      color="neutral"
      variant="ghost"
      size="xs"
      square
      :disabled="!command.changed"
      :aria-label="t('settings.shortcuts.reset', { command: title })"
      :title="t('settings.shortcuts.reset', { command: title })"
      data-testid="reset"
      @click="reset"
    />
    <p v-for="binding in dropped" :key="binding" class="basis-full text-xs text-warning" data-testid="dropped">
      {{ t('settings.shortcuts.dropped', { binding }) }}
    </p>
    <p v-if="refusal" class="basis-full text-xs text-error" role="alert" data-testid="refusal">{{ refusal }}</p>
    <ConflictPrompt
      v-if="conflict"
      :binding="conflict.binding"
      :holder="t(conflict.holder.titleKey)"
      :old="command.bindings[0]"
      @choose="resolve"
    />
  </div>
</template>
