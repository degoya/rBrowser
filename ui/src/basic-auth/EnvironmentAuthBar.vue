<script setup lang="ts">
// Basic auth per environment (RB-094-08): `environment.auth` without arguments (`Leader E A`, the
// Environments menu, the command palette) reaches the shell as `uiCommand`, and this bar covers
// the toolbar strip (the shell view, never a page) with the environment, the user and the
// password, Save and Remove. The password field is cleared as soon as it was sent and whenever
// the bar closes; nothing here keeps or shows a stored password. Settings opens the list of
// environments whose challenge is answered (`basicAuth.environments`, a person's choice). Escape
// closes; the focus goes back to its opener (`returnFocus.ts`, RB-090-23). Mounted in `App.vue`.
// Nuxt UI: `UButton`s for the environments (a select's popover would be cut off by the strip),
// `UInput`s for user and password.
import { computed, nextTick, onScopeDispose, ref, useId } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import UInput from '@nuxt/ui/components/Input.vue';
import { returnFocus } from '../focus/returnFocus';
import { useShell } from '../state/shell';
import { windowEvents, type Subscribe } from '../whichkey/whichkey';
import { AUTH_COMMAND, draftProblem, removeMessage, saveMessage, settingsMessage } from './basicAuth';

const props = withDefaults(defineProps<{ subscribe?: Subscribe }>(), { subscribe: undefined });
const shell = useShell();
const { t } = useI18n();
const id = useId();

const open = ref(false);
const environment = ref('');
const user = ref('');
const password = ref('');
const problem = ref<string | null>(null);
const busy = ref(false);
const userField = ref<{ inputRef?: HTMLInputElement | null } | null>(null);
const focus = returnFocus();
const environments = computed(() => shell.state.value?.environments ?? []);

const stop = (props.subscribe ?? windowEvents)((message) => {
  if (message.type !== 'uiCommand' || message.id !== AUTH_COMMAND) {
    return;
  }
  const shown = shell.state.value?.environment ?? null;
  environment.value = environments.value.find((env) => env.id === shown)?.id ?? environments.value[0]?.id ?? '';
  user.value = '';
  password.value = '';
  problem.value = null;
  focus.remember();
  open.value = true;
  void nextTick(() => userField.value?.inputRef?.focus());
});
onScopeDispose(stop);

function close(): void {
  password.value = '';
  open.value = false;
  focus.restore();
}

async function save(): Promise<void> {
  const draft = { environment: environment.value, user: user.value, password: password.value };
  problem.value = draftProblem(draft);
  if (problem.value !== null) {
    return;
  }
  busy.value = true;
  // Sent once; the field is empty again whatever the answer.
  password.value = '';
  const saved = await shell.send(saveMessage(draft));
  busy.value = false;
  if (saved) {
    close();
  }
}

async function remove(): Promise<void> {
  if (environment.value === '') {
    problem.value = 'auth.bar.problem.environment';
    return;
  }
  busy.value = true;
  const removed = await shell.send(removeMessage(environment.value));
  busy.value = false;
  if (removed) {
    close();
  }
}

async function openSettings(): Promise<void> {
  if (await shell.send(settingsMessage())) {
    close();
  }
}
</script>

<template>
  <form
    v-if="open"
    role="dialog"
    :aria-label="t('commands.environment.auth')"
    class="fixed inset-x-0 top-0 z-50 flex h-11 items-center gap-2 overflow-x-auto border-b border-default bg-elevated px-3 text-sm text-default"
    data-testid="basic-auth-bar"
    @submit.prevent="save"
    @keydown.escape.prevent="close"
  >
    <span :id="`${id}-env`" class="shrink-0 text-muted">{{ t('auth.bar.environment') }}</span>
    <div role="group" :aria-labelledby="`${id}-env`" class="flex shrink-0 items-center gap-1">
      <UButton
        v-for="env in environments"
        :key="env.id"
        type="button"
        size="sm"
        :color="env.id === environment ? 'primary' : 'neutral'"
        :variant="env.id === environment ? 'soft' : 'outline'"
        class="shrink-0"
        :aria-pressed="env.id === environment ? 'true' : 'false'"
        :data-testid="`basic-auth-env-${env.id}`"
        @click="environment = env.id"
      >
        {{ env.id }}
      </UButton>
    </div>
    <label :for="`${id}-user`" class="shrink-0 text-muted">{{ t('auth.bar.user') }}</label>
    <UInput
      :id="`${id}-user`"
      ref="userField"
      v-model="user"
      type="text"
      size="sm"
      class="w-36 shrink-0"
      autocomplete="off"
      spellcheck="false"
      maxlength="256"
      data-testid="basic-auth-user"
    />
    <label :for="`${id}-password`" class="shrink-0 text-muted">{{ t('auth.bar.password') }}</label>
    <UInput
      :id="`${id}-password`"
      v-model="password"
      type="password"
      size="sm"
      class="w-36 shrink-0"
      autocomplete="new-password"
      spellcheck="false"
      maxlength="1024"
      data-testid="basic-auth-password"
    />
    <UButton type="submit" size="sm" class="shrink-0" :loading="busy" :disabled="busy" data-testid="basic-auth-save">
      {{ t('auth.bar.save') }}
    </UButton>
    <UButton
      type="button"
      size="sm"
      color="error"
      variant="outline"
      class="shrink-0"
      :disabled="busy"
      data-testid="basic-auth-remove"
      @click="remove"
    >
      {{ t('auth.bar.remove') }}
    </UButton>
    <span
      class="min-w-0 truncate text-xs"
      :class="problem ? 'text-error' : 'text-muted'"
      role="status"
      data-testid="basic-auth-hint"
    >
      {{ problem ? t(problem) : t('auth.bar.hint') }}
    </span>
    <UButton type="button" size="sm" color="neutral" variant="link" class="shrink-0" data-testid="basic-auth-settings" @click="openSettings">
      {{ t('auth.bar.settings') }}
    </UButton>
    <UButton type="button" size="sm" color="neutral" variant="ghost" class="ml-auto shrink-0" data-testid="basic-auth-cancel" @click="close">
      {{ t('auth.bar.cancel') }}
    </UButton>
  </form>
</template>
