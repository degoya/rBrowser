<script setup lang="ts">
// The toolbar's trust banner (RB-010-09): shown while the open project has protected parts it
// is not trusted with; it opens the trust dialog, which alone can trust.
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import { useTrust } from './trust';

const trust = useTrust();
const { t } = useI18n();

const blocked = computed(() => trust.prompt.value?.blocked.length ?? 0);
</script>

<template>
  <UButton
    v-if="blocked > 0"
    color="warning"
    variant="soft"
    size="sm"
    class="shrink-0"
    :title="t('trust.badge.title')"
    data-testid="trust-badge"
    @click="trust.openDialog()"
  >
    {{ t('trust.badge.restricted', { count: blocked }) }}
  </UButton>
</template>
