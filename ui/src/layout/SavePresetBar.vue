<script setup lang="ts">
// "Save layout as preset" (RB-080-04): `layout.save_preset` without a name (the toolbar's
// bookmark button, `Leader L N`, the command palette) reaches the shell as `uiCommand`, and this
// bar covers the toolbar strip (the shell view, never a page - a popover below the strip would be
// clipped) with the name, "share with the team" and Save. The name starts as the selected
// preset's, else the mode's; sharing starts as `layoutModes.savePresetTo` says. Enter saves,
// Escape closes; a refusal reaches the status bar and the bar stays open. Mounted in `App.vue`.
// Closed by Escape, Cancel or done, the focus goes back to its opener (`returnFocus.ts`, RB-090-23).
// Nuxt UI since RB-080-88: `UInput`, a `USwitch` for sharing (no popover - the bar is a strip),
// `UButton`s.
import { computed, nextTick, onScopeDispose, ref, useId } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import UInput from '@nuxt/ui/components/Input.vue';
import USwitch from '@nuxt/ui/components/Switch.vue';
import { returnFocus } from '../focus/returnFocus';
import { useShell } from '../state/shell';
import { windowEvents, type Subscribe } from '../whichkey/whichkey';
import { SAVE_PRESET_COMMAND, presetName, sharesByDefault, suggestedName } from './savePreset';

const props = withDefaults(defineProps<{ subscribe?: Subscribe }>(), { subscribe: undefined });
const shell = useShell();
const { t } = useI18n();
const id = useId();

const open = ref(false);
const name = ref('');
const share = ref(false);
const invalid = ref(false);
const saving = ref(false);
const field = ref<{ inputRef?: HTMLInputElement | null } | null>(null);
const focus = returnFocus();
const hint = computed(() => (invalid.value ? t('layout.save_preset.invalid') : t('layout.save_preset.hint')));

const stop = (props.subscribe ?? windowEvents)((message) => {
  if (message.type === 'uiCommand' && message.id === SAVE_PRESET_COMMAND) {
    name.value = suggestedName(shell.state.value?.layout);
    share.value = sharesByDefault(shell.settings.value);
    invalid.value = false;
    focus.remember();
    open.value = true;
    void nextTick(() => {
      field.value?.inputRef?.focus();
      field.value?.inputRef?.select();
    });
  }
});
onScopeDispose(stop);

function close(): void {
  open.value = false;
  focus.restore();
}

async function save(): Promise<void> {
  const chosen = presetName(name.value);
  if (chosen === null) {
    invalid.value = true;
    return;
  }
  invalid.value = false;
  saving.value = true;
  const saved = await shell.run({
    command: SAVE_PRESET_COMMAND,
    args: { id: chosen, file: share.value ? 'project' : 'local' },
  });
  saving.value = false;
  if (saved) {
    close();
  }
}
</script>

<template>
  <form
    v-if="open"
    role="dialog"
    :aria-label="t('shell.toolbar.save_preset')"
    class="fixed inset-x-0 top-0 z-50 flex h-11 items-center gap-2 overflow-x-auto border-b border-default bg-elevated px-3 text-sm text-default"
    data-testid="save-preset"
    @submit.prevent="save"
    @keydown.escape.prevent="close"
  >
    <label :for="`${id}-name`" class="shrink-0 text-muted">{{ t('layout.save_preset.label') }}</label>
    <UInput
      :id="`${id}-name`"
      ref="field"
      v-model="name"
      type="text"
      size="sm"
      class="w-48 shrink-0"
      autocomplete="off"
      spellcheck="false"
      maxlength="60"
      :color="invalid ? 'error' : 'primary'"
      :highlight="invalid"
      :aria-invalid="invalid ? 'true' : undefined"
      :aria-describedby="`${id}-hint`"
      data-testid="save-preset-name"
    />
    <USwitch
      v-model="share"
      size="sm"
      class="shrink-0"
      :label="t('layout.save_preset.share')"
      :ui="{ label: 'text-muted font-normal' }"
      data-testid="save-preset-share"
    />
    <UButton type="submit" size="sm" class="shrink-0" :loading="saving" :disabled="saving" data-testid="save-preset-save">
      {{ t('layout.save_preset.save') }}
    </UButton>
    <span
      :id="`${id}-hint`"
      class="min-w-0 truncate text-xs"
      :class="invalid ? 'text-error' : 'text-muted'"
      data-testid="save-preset-hint"
    >
      {{ hint }}
    </span>
    <UButton
      size="sm"
      color="neutral"
      variant="ghost"
      class="ml-auto shrink-0"
      data-testid="save-preset-cancel"
      @click="close"
    >
      {{ t('layout.save_preset.cancel') }}
    </UButton>
  </form>
</template>
