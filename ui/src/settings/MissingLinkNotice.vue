<script setup lang="ts">
// The notice at the top of the settings for a link that opened no setting (RB-094-26): it names
// the key the link asked for, so a link never silently shows another page; while the plugins
// still load it says the key opens once they are in. A Nuxt UI `UAlert` with a close button.
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import UAlert from '@nuxt/ui/components/Alert.vue';
import UButton from '@nuxt/ui/components/Button.vue';
import { SETTINGS_ICONS } from './icons';
import type { MissingLink } from './route';

const props = defineProps<{ link: MissingLink }>();
const emit = defineEmits<{ dismiss: [] }>();
const { t } = useI18n();
const text = computed(() => {
  if (props.link.key === '') return t('settings.link.not_found_unnamed');
  return t(props.link.waiting ? 'settings.link.waiting' : 'settings.link.not_found', { key: props.link.key });
});
</script>

<template>
  <UAlert
    :role="link.waiting ? 'status' : 'alert'"
    :color="link.waiting ? 'info' : 'warning'"
    variant="subtle"
    orientation="horizontal"
    :icon="link.waiting ? SETTINGS_ICONS.loading : SETTINGS_ICONS.warning"
    :description="text"
    data-testid="missing-link"
  >
    <template #actions>
      <UButton
        :icon="SETTINGS_ICONS.close"
        color="neutral"
        variant="ghost"
        size="xs"
        :aria-label="t('settings.link.close')"
        :title="t('settings.link.close')"
        data-testid="missing-link-close"
        @click="emit('dismiss')"
      />
    </template>
  </UAlert>
</template>
