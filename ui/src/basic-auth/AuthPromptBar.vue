<script setup lang="ts">
// rBrowser's own sign-in dialog (RB-099-04): Chromium's prompt is off, so a challenge the stored
// credentials do not answer - another site's, a proxy's, a refused password's - is asked here,
// in the shell's strip, never in a page. The bar names the origin, the realm, a proxy and the
// views waiting; Sign in answers all of them at once, Cancel or Escape leaves them to the
// server's 401 page. A plain-http question says the answer is sent unencrypted (review w35).
// "Save for environment" is offered only when the core names the environment
// whose address this is; otherwise the answer is used once (Chromium keeps it for the session).
// Questions queue, one per origin, realm and proxy. The password field is cleared as soon as it
// was sent and whenever the question changes; nothing here keeps a password. The bar is a named
// form, not a modal dialog (the views stay usable); a problem is said at once (`role="alert"`,
// in the DOM before its first word) and marks the field it is about. A password manager may
// fill the fields (`username`, `current-password`; owner, RB-099-21). Mounted in `App.vue`.
// Nuxt UI: `UInput`, `UCheckbox`, `UButton`.
import { computed, nextTick, onScopeDispose, ref, useId, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import UCheckbox from '@nuxt/ui/components/Checkbox.vue';
import UInput from '@nuxt/ui/components/Input.vue';
import type { AuthPrompt } from '../ipc/protocol';
import { returnFocus } from '../focus/returnFocus';
import { useShell } from '../state/shell';
import { windowEvents, type Subscribe } from '../whichkey/whichkey';
import { nextQueue, problemField, replyMessage, signInProblem, viewNames } from './authPrompt';

const props = withDefaults(defineProps<{ subscribe?: Subscribe }>(), { subscribe: undefined });
const shell = useShell();
const { t } = useI18n();
const id = useId();

const queue = ref<AuthPrompt[]>([]);
const prompt = computed<AuthPrompt | null>(() => queue.value[0] ?? null);
const user = ref('');
const password = ref('');
const save = ref(false);
const problem = ref<string | null>(null);
const busy = ref(false);
const userField = ref<{ inputRef?: HTMLInputElement | null } | null>(null);
const focus = returnFocus();

const title = computed(() => {
  const open = prompt.value;
  if (open === null) {
    return '';
  }
  return t(open.proxy ? 'auth.prompt.proxy' : 'auth.prompt.site', { origin: open.origin });
});
const problemId = `${id}-problem`;
/** The field the problem is about: `aria-invalid` and `aria-describedby` on it alone. */
const invalid = computed(() => problemField(problem.value));
const fieldProblem = (field: 'user' | 'password') =>
  invalid.value === field ? { 'aria-invalid': 'true' as const, 'aria-describedby': problemId } : {};
const waiting = computed(() =>
  prompt.value === null ? '' : viewNames(prompt.value.views, shell.state.value?.views ?? []),
);

const stop = (props.subscribe ?? windowEvents)((message) => {
  const next = nextQueue(queue.value, message);
  if (next !== null) {
    queue.value = next;
  }
});
onScopeDispose(stop);

// A new question in front: empty fields, the focus in the user field.
watch(
  () => prompt.value?.id ?? null,
  (current, before) => {
    if (current === before) {
      return;
    }
    user.value = '';
    password.value = '';
    save.value = false;
    problem.value = null;
    busy.value = false;
    if (current === null) {
      focus.restore();
      return;
    }
    if (before === null) {
      focus.remember();
    }
    void nextTick(() => userField.value?.inputRef?.focus());
  },
);

async function answer(signIn: boolean): Promise<void> {
  const open = prompt.value;
  if (open === null || busy.value) {
    return;
  }
  const credential = { user: user.value, password: password.value, save: save.value && open.environment !== undefined };
  if (signIn) {
    problem.value = signInProblem(credential);
    if (problem.value !== null) {
      return;
    }
  }
  busy.value = true;
  // Sent once; the field is empty again whatever the answer.
  password.value = '';
  const sent = await shell.send(replyMessage(open.id, signIn ? credential : undefined));
  busy.value = false;
  // A refused save keeps the question (the status bar says why); a cancel always ends it.
  if (sent || !signIn) {
    queue.value = queue.value.filter((queued) => queued.id !== open.id);
  }
}
</script>

<template>
  <form
    v-if="prompt !== null"
    role="form"
    :aria-label="title"
    class="fixed inset-x-0 top-0 z-50 flex h-11 items-center gap-2 overflow-x-auto border-b border-default bg-elevated px-3 text-sm text-default"
    data-testid="auth-prompt"
    @submit.prevent="answer(true)"
    @keydown.escape.prevent="answer(false)"
  >
    <span class="shrink-0 font-medium" data-testid="auth-prompt-title">{{ title }}</span>
    <span v-if="prompt.realm" class="shrink-0 text-xs text-muted" data-testid="auth-prompt-realm">{{
      t('auth.prompt.realm', { realm: prompt.realm })
    }}</span>
    <span v-if="waiting !== ''" class="shrink-0 text-xs text-muted" data-testid="auth-prompt-views">{{
      t('auth.prompt.views', { views: waiting })
    }}</span>
    <span v-if="prompt.refused" class="shrink-0 text-xs text-error" role="status" data-testid="auth-prompt-refused">{{
      t('auth.prompt.refused')
    }}</span>
    <span v-if="prompt.insecure" class="shrink-0 text-xs text-warning" data-testid="auth-prompt-insecure">{{
      t('auth.prompt.insecure')
    }}</span>
    <label :for="`${id}-user`" class="shrink-0 text-muted">{{ t('auth.bar.user') }}</label>
    <UInput
      :id="`${id}-user`"
      ref="userField"
      v-model="user"
      type="text"
      size="sm"
      class="w-36 shrink-0"
      autocomplete="username"
      spellcheck="false"
      maxlength="256"
      v-bind="fieldProblem('user')"
      data-testid="auth-prompt-user"
    />
    <label :for="`${id}-password`" class="shrink-0 text-muted">{{ t('auth.bar.password') }}</label>
    <UInput
      :id="`${id}-password`"
      v-model="password"
      type="password"
      size="sm"
      class="w-36 shrink-0"
      autocomplete="current-password"
      spellcheck="false"
      maxlength="1024"
      v-bind="fieldProblem('password')"
      data-testid="auth-prompt-password"
    />
    <UCheckbox
      v-if="prompt.environment !== undefined"
      v-model="save"
      class="shrink-0"
      :label="t('auth.prompt.save', { environment: prompt.environment })"
      data-testid="auth-prompt-save"
    />
    <UButton type="submit" size="sm" class="shrink-0" :loading="busy" :disabled="busy" data-testid="auth-prompt-sign-in">
      {{ t('auth.prompt.signIn') }}
    </UButton>
    <UButton
      type="button"
      size="sm"
      color="neutral"
      variant="ghost"
      class="shrink-0"
      :disabled="busy"
      data-testid="auth-prompt-cancel"
      @click="answer(false)"
    >
      {{ t('auth.bar.cancel') }}
    </UButton>
    <span :id="problemId" class="min-w-0 truncate text-xs text-error" role="alert" data-testid="auth-prompt-problem">{{
      problem ? t(problem) : ''
    }}</span>
    <span v-if="queue.length > 1" class="ml-auto shrink-0 text-xs text-muted" data-testid="auth-prompt-queued">{{
      t('auth.prompt.queued', { count: queue.length - 1 })
    }}</span>
  </form>
</template>
