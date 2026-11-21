<script setup lang="ts">
// The request's auth (RB-096-01): basic, bearer and an API key (header or query) are sent;
// digest and OAuth 2 are kept in the file and not sent yet. A secret belongs in a variable -
// `{{token}}` from the private values or `{{$secret token}}` (RB_SECRET_TOKEN) - never in the
// checked-in file.
import { computed } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { AUTH_NOT_SENT, AUTH_TYPES, type Auth, type AuthType, withAuthType } from './model';

const auth = defineModel<Auth>({ required: true });
/** How a secret is written in a request: forms of the file, not texts to translate. */
const SECRET_EXAMPLES = { variable: '{{token}}', secret: '{{$secret token}}' };
const t = usePanel().t;

const typeItems = computed(() => AUTH_TYPES.map((type) => ({ label: t(`panel.auth.types.${type}`), value: type })));
const placeItems = computed(() => [
  { label: t('panel.auth.inHeader'), value: 'header' },
  { label: t('panel.auth.inQuery'), value: 'query' },
]);
const grantItems = computed(() => [
  { label: t('panel.auth.clientCredentials'), value: 'clientCredentials' },
  { label: t('panel.auth.authorizationCode'), value: 'authorizationCode' },
]);
const type = computed<AuthType>({
  get: () => auth.value.type,
  set: (next) => {
    auth.value = withAuthType(auth.value, next);
  },
});

/** `auth` with `field` set to `value`. */
function set(field: string, value: unknown): void {
  auth.value = { ...auth.value, [field]: value } as Auth;
}

/** The text of `field`, empty when the type has none. */
function text(field: string): string {
  const value = (auth.value as Record<string, unknown>)[field];
  return typeof value === 'string' ? value : '';
}

const fields = computed<string[]>(() => {
  switch (auth.value.type) {
    case 'basic':
    case 'digest':
      return ['username', 'password'];
    case 'bearer':
      return ['token'];
    case 'apiKey':
      return ['name', 'value'];
    case 'oauth2':
      return auth.value.grant === 'authorizationCode'
        ? ['tokenUrl', 'authUrl', 'clientId', 'clientSecret', 'scope', 'redirectUri']
        : ['tokenUrl', 'clientId', 'clientSecret', 'scope'];
    default:
      return [];
  }
});
</script>

<template>
  <div id="auth" class="flex flex-col gap-2 pt-2">
    <USelect id="auth-type" v-model="type" :items="typeItems" value-key="value" size="sm" class="w-48" :aria-label="t('panel.auth.type')" />
    <USelect
      v-if="auth.type === 'apiKey'"
      id="auth-place"
      :model-value="auth.in"
      :items="placeItems"
      value-key="value"
      size="sm"
      class="w-48"
      :aria-label="t('panel.auth.place')"
      @update:model-value="(place) => set('in', place)"
    />
    <USelect
      v-if="auth.type === 'oauth2'"
      id="auth-grant"
      :model-value="auth.grant"
      :items="grantItems"
      value-key="value"
      size="sm"
      class="w-64"
      :aria-label="t('panel.auth.grant')"
      @update:model-value="(grant) => set('grant', grant)"
    />
    <UFormField v-for="field in fields" :key="field" :label="t(`panel.auth.fields.${field}`)" :name="`auth-${field}`" size="sm">
      <UInput
        :id="`auth-${field}`"
        :model-value="text(field)"
        size="sm"
        class="w-full font-mono"
        spellcheck="false"
        autocomplete="off"
        @update:model-value="(value) => set(field, String(value))"
      />
    </UFormField>
    <p v-if="auth.type !== 'none'" id="auth-hint" class="text-xs text-muted">{{ t('panel.auth.secretHint', SECRET_EXAMPLES) }}</p>
    <UAlert
      v-if="AUTH_NOT_SENT.includes(auth.type)"
      id="auth-not-sent"
      color="warning"
      variant="subtle"
      icon="i-lucide-triangle-alert"
      :title="t('panel.auth.notSent')"
      :ui="{ root: 'px-3 py-1.5', title: 'text-sm font-normal' }"
    />
  </div>
</template>
