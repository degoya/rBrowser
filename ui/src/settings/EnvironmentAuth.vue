<script setup lang="ts">
// Basic auth per environment under `basicAuth.environments` in Settings > Project > Environments
// (RB-094-24): each environment's switch (whether rBrowser answers its sign-in prompt, the key's
// list), its user and password, Save and Remove. The credential goes through `environment.auth`,
// the command of the shell's bar (`Leader E A`), so both store it in the same place - the
// platform's credential store, bound to the project and the environment's address, never a
// project file - and both show the same switch. The password field is sent once and cleared;
// nothing here keeps or shows a stored password. Each row says whether a credential is stored and
// for which user (`getBasicAuthStatus`, asked at the start and after Save and Remove, followed
// when the core pushes a change; RB-094-29); Remove is off without one. Environments with one
// address share one credential and one sign-in, and their rows say with which; the answer of Save
// or Remove stands on its row's own line under the controls, beside the stored user (RB-099-04).
// Nuxt UI: `USwitch`, `UInput`, `UButton`.
import { computed, onScopeDispose, reactive, ref, useId } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import UInput from '@nuxt/ui/components/Input.vue';
import USwitch from '@nuxt/ui/components/Switch.vue';
import type { IpcClient } from '../ipc/client';
import type { JsonValue } from '../ipc/protocol';
import {
  canRemove,
  draftProblem,
  removeMessage,
  saveMessage,
  statusesOf,
  storeParams,
  storedText,
  storedUsers,
  usersOf,
  type StoredUsers,
} from '../basic-auth/basicAuth';
import { environmentIdsOf, sharingOf, withListed } from './environmentAuth';

const props = defineProps<{ client: IpcClient; environments: JsonValue; listed: JsonValue }>();
const emit = defineEmits<{ set: [value: string[]] }>();
const { t } = useI18n();
const id = useId();

const ids = computed(() => environmentIdsOf(props.environments));
const sharing = computed(() => sharingOf(props.environments));
const users = reactive<Record<string, string>>({});
const passwords = reactive<Record<string, string>>({});
/** The i18n key of what the row says: a problem, stored or removed. */
const status = reactive<Record<string, string>>({});
const busy = reactive<Record<string, boolean>>({});
/** The stored users by environment; `null` until the core said. */
const storedBy = ref<StoredUsers | null>(null);

/** Asks the core what is stored; a core that cannot say leaves the lines empty and Remove on. */
async function refresh(): Promise<void> {
  try {
    const answer = await props.client.expect({ type: 'getBasicAuthStatus' }, 'basicAuthStatus');
    storedBy.value = storedUsers(statusesOf(answer));
  } catch {
    storedBy.value = null;
  }
}

onScopeDispose(
  props.client.on((message) => {
    const pushed = usersOf(message);
    if (pushed !== null) {
      storedBy.value = pushed;
    }
  }),
);
void refresh();

function storedLine(env: string): string {
  const text = storedText(storedBy.value, env);
  return text === null ? '' : t(text.key, text.params);
}

/** What the row says about the environments sharing its address; empty for none. */
function sharedLine(env: string): string {
  const others = sharing.value[env] ?? [];
  return others.length === 0 ? '' : t('auth.settings.shared', { environments: others.join(', ') });
}

function statusText(env: string): string {
  const key = status[env];
  return key === undefined ? '' : t(key, storeParams(t));
}

function isProblem(env: string): boolean {
  return status[env]?.startsWith('auth.bar.problem') === true;
}

function isListed(env: string): boolean {
  return Array.isArray(props.listed) && props.listed.includes(env);
}

function onSwitch(env: string, on: boolean): void {
  emit('set', withListed(props.listed, env, on));
}

async function send(env: string, message: Parameters<IpcClient['request']>[0], done: string): Promise<void> {
  busy[env] = true;
  try {
    await props.client.request(message);
    status[env] = done;
  } catch {
    status[env] = 'auth.settings.failed';
  } finally {
    busy[env] = false;
  }
  await refresh();
}

async function save(env: string): Promise<void> {
  const draft = { environment: env, user: users[env] ?? '', password: passwords[env] ?? '' };
  const problem = draftProblem(draft);
  if (problem !== null) {
    status[env] = problem;
    return;
  }
  // Sent once; the field is empty again whatever the answer.
  passwords[env] = '';
  await send(env, saveMessage(draft), 'auth.settings.saved');
}

async function remove(env: string): Promise<void> {
  passwords[env] = '';
  await send(env, removeMessage(env), 'auth.settings.removed');
}
</script>

<template>
  <div class="flex flex-col gap-2" data-testid="env-auth">
    <p class="text-xs text-muted">{{ t('auth.bar.hint', storeParams(t)) }}</p>
    <div class="flex flex-col divide-y divide-default">
      <form
        v-for="env in ids"
        :key="env"
        class="flex flex-col gap-1 py-2"
        :aria-label="t('auth.settings.row', { environment: env })"
        :data-testid="`env-auth-${env}`"
        @submit.prevent="save(env)"
      >
        <div class="flex flex-wrap items-center gap-2" :data-testid="`env-auth-controls-${env}`">
          <USwitch
            :model-value="isListed(env)"
            :label="env"
            class="w-40 shrink-0"
            :data-testid="`env-auth-switch-${env}`"
            @update:model-value="(on: boolean) => onSwitch(env, on)"
          />
          <label :for="`${id}-${env}-user`" class="text-xs text-muted">{{ t('auth.bar.user') }}</label>
          <UInput
            :id="`${id}-${env}-user`"
            v-model="users[env]"
            type="text"
            size="sm"
            class="w-36"
            autocomplete="off"
            spellcheck="false"
            maxlength="256"
            :data-testid="`env-auth-user-${env}`"
          />
          <label :for="`${id}-${env}-password`" class="text-xs text-muted">{{ t('auth.bar.password') }}</label>
          <UInput
            :id="`${id}-${env}-password`"
            v-model="passwords[env]"
            type="password"
            size="sm"
            class="w-36"
            autocomplete="new-password"
            spellcheck="false"
            maxlength="1024"
            :data-testid="`env-auth-password-${env}`"
          />
          <UButton
            type="button"
            size="sm"
            :loading="busy[env]"
            :disabled="busy[env]"
            :data-testid="`env-auth-save-${env}`"
            @click="save(env)"
          >
            {{ t('auth.bar.save') }}
          </UButton>
          <UButton
            type="button"
            size="sm"
            color="error"
            variant="outline"
            :disabled="busy[env] || !canRemove(storedBy, env)"
            :data-testid="`env-auth-remove-${env}`"
            @click="remove(env)"
          >
            {{ t('auth.bar.remove') }}
          </UButton>
        </div>
        <!-- The row's own line, aligned under its fields: the stored user, the shared address and
             the answer of Save or Remove, so no answer reads as a gap between two rows. -->
        <div
          class="flex flex-wrap items-baseline gap-x-3 gap-y-1 ps-42 text-xs"
          :data-testid="`env-auth-info-${env}`"
        >
          <span class="text-default" aria-live="polite" :data-testid="`env-auth-stored-${env}`">{{
            storedLine(env)
          }}</span>
          <span v-if="sharedLine(env) !== ''" class="text-muted" :data-testid="`env-auth-shared-${env}`">{{
            sharedLine(env)
          }}</span>
          <span
            class="min-w-0"
            :class="isProblem(env) ? 'text-error' : 'text-muted'"
            role="status"
            :data-testid="`env-auth-status-${env}`"
          >{{ statusText(env) }}</span>
        </div>
      </form>
    </div>
  </div>
</template>
