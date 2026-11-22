<script setup lang="ts">
// The notice at the top of the settings for a write the core refused (RB-094-25): the setting,
// every reason, a button that opens the field and one that closes the notice. The reasons stay
// under the field as well; there they may lie below a long list, out of sight, and a confirmed
// Delete that changed nothing looked like it did nothing. A Nuxt UI `UAlert`.
import { useI18n } from 'vue-i18n';
import UAlert from '@nuxt/ui/components/Alert.vue';
import UButton from '@nuxt/ui/components/Button.vue';
import { SETTINGS_ICONS } from './icons';
import type { Refusal } from './store';

defineProps<{ refusal: Refusal; label: string }>();
const emit = defineEmits<{ show: [key: string]; dismiss: [] }>();
const { t } = useI18n();
</script>

<template>
  <UAlert
    role="alert"
    color="error"
    variant="subtle"
    :icon="SETTINGS_ICONS.warning"
    :title="t('settings.refused.title', { label })"
    data-testid="refusal-notice"
  >
    <template #description>
      <ul class="flex flex-col gap-0.5">
        <li v-for="(issue, index) in refusal.issues" :key="index" data-testid="refusal-reason">
          <span v-if="issue.path !== ''" class="font-mono">{{ issue.path }}: </span>{{ t(issue.messageKey) }}
        </li>
      </ul>
    </template>
    <template #actions>
      <UButton
        color="error"
        variant="soft"
        size="xs"
        :label="t('settings.refused.show')"
        data-testid="refusal-show"
        @click="emit('show', refusal.key)"
      />
      <UButton
        :icon="SETTINGS_ICONS.close"
        color="neutral"
        variant="ghost"
        size="xs"
        :aria-label="t('settings.refused.close')"
        :title="t('settings.refused.close')"
        data-testid="refusal-close"
        @click="emit('dismiss')"
      />
    </template>
  </UAlert>
</template>
