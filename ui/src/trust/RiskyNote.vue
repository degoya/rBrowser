<script setup lang="ts">
// The badge beside a risky grant in the trust dialog (RB-090-22): the word and, where the grant's
// own text does not say it, the reason - so the risk is told without its colour and without
// hovering for a title (WCAG 1.4.1).
import { useI18n } from 'vue-i18n';
import UBadge from '@nuxt/ui/components/Badge.vue';

defineProps<{ reason?: string }>();
const { t } = useI18n();
</script>

<template>
  <UBadge
    color="warning"
    variant="subtle"
    size="sm"
    class="ms-1 whitespace-normal align-middle font-normal"
    data-testid="trust-risky-note"
    :label="reason ? t('trust.page.grants.risky_because', { reason }) : t('trust.page.grants.risky_label')"
  />
</template>
