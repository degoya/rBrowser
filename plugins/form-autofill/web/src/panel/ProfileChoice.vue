<script setup lang="ts">
// The profile Fill valid uses (RB-090-71), always in sight at the top of the panel: "No profile",
// the settings' profiles and the presets taken from forms, each group under its own heading. A
// preset chosen can be renamed (an inline name box) or deleted (an inline confirmation, a panel
// opens no modal - docs/design.md section 6) through the tools `rename` and `delete`, which touch
// only the plugin's own presets file; a refusal stays here as one sentence. "Fill with <name>"
// beside the choice fills the views with the profile chosen (RB-090-72: a choice alone did nothing
// in sight); the hint names what Fill valid and Fill invalid take.
import { computed, nextTick, ref, watch } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import { MAX_NAME } from './capture';
import { profileGroups, renameState } from './presetChoice';

const props = defineProps<{ profiles: string[]; presets: string[]; filling?: boolean }>();
const profile = defineModel<string>({ required: true });
const emit = defineEmits<{ renamed: [{ from: string; to: string }]; deleted: [string]; fill: [] }>();
const panel = usePanel();
const t = panel.t;

// A choice cannot carry an empty value: this stands for "no profile".
const NONE = '\u0000none';

const items = computed(() => profileGroups(t, props.profiles, props.presets, NONE));
const chosen = computed({
  get: () => profile.value || NONE,
  set: (value: string) => {
    profile.value = value === NONE ? '' : value;
  },
});
const hint = computed(() =>
  profile.value ? t('panel.profileHintChosen', { name: profile.value }) : t('panel.profileHint'),
);
/** The preset chosen, when the choice is one (a settings profile of the name wins). */
const preset = computed(() => (props.presets.includes(profile.value) && !props.profiles.includes(profile.value) ? profile.value : null));

const editing = ref<'rename' | 'delete' | null>(null);
const name = ref('');
const busy = ref(false);
const refusal = ref<string | null>(null);
const nameInput = ref<{ inputRef?: HTMLInputElement } | null>(null);
const state = computed(() => (preset.value ? renameState(name.value, preset.value, props.profiles, props.presets) : 'empty'));
const canRename = computed(() => !busy.value && state.value === 'ok');

// Another choice ends an edit begun.
watch(preset, () => {
  editing.value = null;
  refusal.value = null;
});

async function startRename(): Promise<void> {
  name.value = preset.value ?? '';
  refusal.value = null;
  editing.value = 'rename';
  await nextTick();
  nameInput.value?.inputRef?.select();
}

function startDelete(): void {
  refusal.value = null;
  editing.value = 'delete';
}

function cancel(): void {
  editing.value = null;
  refusal.value = null;
}

async function rename(): Promise<void> {
  const from = preset.value;
  if (!from || !canRename.value) return;
  const to = name.value.trim();
  busy.value = true;
  refusal.value = null;
  try {
    await panel.call({ tool: 'rename' }, { name: from, to });
    editing.value = null;
    emit('renamed', { from, to });
  } catch (error) {
    refusal.value = t('panel.choice.renameFailed', { text: panel.error(error) });
  } finally {
    busy.value = false;
  }
}

async function remove(): Promise<void> {
  const gone = preset.value;
  if (!gone || busy.value) return;
  busy.value = true;
  refusal.value = null;
  try {
    await panel.call({ tool: 'delete' }, { name: gone });
    editing.value = null;
    emit('deleted', gone);
  } catch (error) {
    refusal.value = t('panel.choice.deleteFailed', { text: panel.error(error) });
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <section data-profile-choice class="flex flex-col gap-2">
    <UFormField :label="t('panel.profile')" :help="hint" class="min-w-0">
      <div class="flex min-w-0 items-center gap-1">
        <USelect id="profile" v-model="chosen" :items="items" size="sm" class="min-w-0 flex-1" :ui="{ itemLabel: 'truncate' }" />
        <UButton
          v-if="profile"
          id="fill-with"
          data-primary
          size="sm"
          icon="i-lucide-circle-check"
          :label="t('panel.choice.fillWith', { name: profile })"
          :loading="filling"
          :disabled="filling"
          class="min-w-0 max-w-1/2"
          :ui="{ label: 'truncate' }"
          @click="emit('fill')"
        />
        <template v-if="preset">
          <IconAction id="rename-preset" icon="i-lucide-pencil" :label="t('panel.choice.rename')" :disabled="busy" @click="startRename" />
          <IconAction id="delete-preset" icon="i-lucide-trash-2" :label="t('panel.choice.delete')" :disabled="busy" @click="startDelete" />
        </template>
      </div>
    </UFormField>

    <div v-if="preset && editing === 'rename'" id="rename-row" class="flex flex-col gap-1" @keydown.esc.stop="cancel">
      <UFieldGroup size="sm" class="w-full">
        <UInput
          id="preset-new-name"
          ref="nameInput"
          v-model="name"
          :maxlength="MAX_NAME"
          :aria-label="t('panel.choice.newName', { name: preset })"
          class="min-w-0 flex-1"
          @keydown.enter.prevent="rename"
        />
        <UButton id="rename-save" :label="t('panel.choice.renameSave')" :loading="busy" :disabled="!canRename" @click="rename" />
        <UButton id="rename-cancel" color="neutral" variant="outline" :label="t('panel.capture.cancel')" @click="cancel" />
      </UFieldGroup>
      <p v-if="state === 'inSettings'" id="rename-in-settings" class="text-xs text-error">{{ t('panel.capture.inSettings', { name: name.trim() }) }}</p>
      <p v-else-if="state === 'taken'" id="rename-taken" class="text-xs text-error">{{ t('panel.choice.taken', { name: name.trim() }) }}</p>
    </div>

    <UAlert
      v-if="preset && editing === 'delete'"
      id="confirm-delete-preset"
      color="error"
      variant="subtle"
      icon="i-lucide-trash-2"
      :title="t('panel.choice.confirmDelete', { name: preset })"
      :ui="{ root: 'px-3 py-2', title: 'text-sm font-normal' }"
      @keydown.esc.stop="cancel"
    >
      <template #actions>
        <UButton id="delete-confirm" size="xs" color="error" :label="t('panel.choice.delete')" :loading="busy" @click="remove" />
        <UButton id="delete-keep" size="xs" color="neutral" variant="outline" :label="t('panel.choice.keep')" @click="cancel" />
      </template>
    </UAlert>

    <p v-if="refusal" id="preset-refused" role="alert" class="text-xs text-error">{{ refusal }}</p>
  </section>
</template>
