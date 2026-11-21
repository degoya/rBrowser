<script setup lang="ts">
// `{{$secret name}}`s of the request without a value (RB-096-11), said before the send: "Set
// value" asks for it and keeps it in the private values (Settings > Network > API client >
// Private values) for this environment or every one, at once and without a restart. An
// RB_SECRET_<NAME> variable of the machine is the other way; a private value comes first.
import { computed, ref } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import SettingsText from './SettingsText.vue';

const props = defineProps<{ missing: string[]; environment: string | undefined }>();
const emit = defineEmits<{ set: [name: string, value: string, environment: string | null] }>();
const t = usePanel().t;

/** Every environment as a select item: an item's value cannot be empty. */
const EVERY = '/';
const editing = ref<string | null>(null);
const value = ref('');
const scope = ref(EVERY);
const scopeItems = computed(() => [
  ...(props.environment ? [{ label: t('panel.secretValue.thisEnvironment', { environment: props.environment }), value: props.environment }] : []),
  { label: t('panel.secretValue.everyEnvironment'), value: EVERY },
]);

function edit(name: string): void {
  editing.value = name;
  value.value = '';
  scope.value = props.environment ?? EVERY;
}

function save(): void {
  if (!editing.value || value.value === '') return;
  emit('set', editing.value, value.value, scope.value === EVERY ? null : scope.value);
  editing.value = null;
  value.value = '';
}
</script>

<template>
  <UAlert v-if="missing.length > 0" id="secrets-missing" color="warning" variant="subtle" icon="i-lucide-lock-open" :title="t('panel.secretValue.title', { names: missing.join(', ') })">
    <template #description>
      <div class="flex flex-col gap-1.5">
        <p class="text-xs">
          <SettingsText text="panel.secretValue.description" setting-key="api-client.secrets" />
        </p>
        <div v-for="name in missing" :key="name" :data-secret-name="name" class="flex flex-wrap items-center gap-1.5">
          <span class="font-mono text-xs">{{ name }}</span>
          <UButton v-if="editing !== name" size="xs" color="neutral" variant="outline" icon="i-lucide-key-round" :label="t('panel.secretValue.set')" :aria-label="t('panel.secretValue.setFor', { name })" @click="edit(name)" />
          <form v-else class="flex flex-wrap items-center gap-1.5" @submit.prevent="save" @keydown.escape.prevent="editing = null">
            <UInput id="secret-value" v-model="value" type="password" size="xs" class="w-44 font-mono" autofocus autocomplete="off" :aria-label="t('panel.secretValue.value', { name })" />
            <USelect id="secret-scope" v-model="scope" :items="scopeItems" value-key="value" size="xs" class="w-44" :aria-label="t('panel.secretValue.scope')" />
            <UButton id="secret-save" type="submit" size="xs" :label="t('panel.save')" :disabled="value === ''" />
            <UButton size="xs" color="neutral" variant="ghost" :label="t('panel.dialog.cancel')" @click="editing = null" />
          </form>
        </div>
      </div>
    </template>
  </UAlert>
</template>
