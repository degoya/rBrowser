<script setup lang="ts">
// Under a key that applies only at the next start of rBrowser (`x-restart`, RB-094-23): says so,
// and once the key was changed in this window offers "Restart now" (`app.restart`, `Leader , S`),
// which closes rBrowser and opens the same project again. Every other key applies at once.
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';

defineProps<{ changed: boolean }>();
const emit = defineEmits<{ restart: [] }>();
const { t } = useI18n();
</script>

<template>
  <div class="flex flex-wrap items-center gap-2" data-testid="restart-notice">
    <span class="text-xs text-warning">{{ t('settings.restart.notice') }}</span>
    <UButton
      v-if="changed"
      size="xs"
      color="warning"
      variant="soft"
      :label="t('settings.restart.now')"
      data-testid="restart-now"
      @click="emit('restart')"
    />
  </div>
</template>
