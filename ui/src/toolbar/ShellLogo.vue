<script setup lang="ts">
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import logoLight from '@assets/rBrowser-logo.svg';
import logoDark from '@assets/rBrowser-logo-dark.svg';

const props = defineProps<{ theme: 'light' | 'dark' }>();
const { t } = useI18n();
const src = computed(() => (props.theme === 'dark' ? logoDark : logoLight));
</script>

<template>
  <img :src="src" :alt="t('shell.toolbar.logo')" class="h-6 w-auto shrink-0 select-none" draggable="false" />
</template>
