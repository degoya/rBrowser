<script setup lang="ts">
// The variables the request in the editor uses (RB-096-11), one badge each below the header:
// pointed at (or focused), a plain one shows its value in the chosen environment, a private one
// says it is kept privately, never its value; a name with no value there is marked, and so is a
// `{{$secret name}}` without one (`SecretsAlert.vue` sets it).
import { computed } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import type { ApiRequest } from './model';
import { type VarBlock, hintOf, references } from './variables';

const props = defineProps<{ request: ApiRequest; environment: string | undefined; blocks: VarBlock[]; baseUrls: Record<string, string>; missing: string[] }>();
const t = usePanel().t;

interface Shown {
  key: string;
  label: string;
  color: 'neutral' | 'error' | 'warning';
  icon: string;
  tip: string;
}

const shown = computed<Shown[]>(() => {
  const used = references(props.request);
  const environment = props.environment ?? '';
  const plain = used.plain.map((name): Shown => {
    const hint = hintOf(name, props.environment, props.blocks, props.baseUrls);
    if (hint.state === 'plain') return { key: name, label: name, color: 'neutral', icon: 'i-lucide-variable', tip: t('panel.hints.value', { name, value: hint.value }) };
    if (hint.state === 'secret') return { key: name, label: name, color: 'neutral', icon: 'i-lucide-lock', tip: t('panel.hints.private', { name }) };
    return { key: name, label: name, color: 'error', icon: 'i-lucide-circle-help', tip: t('panel.hints.unknown', { name, environment }) };
  });
  const secrets = used.secrets.map((name): Shown =>
    props.missing.includes(name)
      ? { key: `$secret ${name}`, label: `$secret ${name}`, color: 'warning', icon: 'i-lucide-lock-open', tip: t('panel.hints.secretMissing', { name }) }
      : { key: `$secret ${name}`, label: `$secret ${name}`, color: 'neutral', icon: 'i-lucide-lock', tip: t('panel.hints.secret', { name }) },
  );
  return [...plain, ...secrets];
});
</script>

<template>
  <ul v-if="shown.length > 0" id="variable-hints" class="flex flex-wrap gap-1" :aria-label="t('panel.hints.title')">
    <li v-for="hint in shown" :key="hint.key">
      <UTooltip :text="hint.tip">
        <UBadge
          :data-hint="hint.key"
          :data-state="hint.color"
          :color="hint.color"
          variant="subtle"
          size="sm"
          :icon="hint.icon"
          :label="hint.label"
          class="font-mono"
          tabindex="0"
          :aria-label="hint.tip"
        />
      </UTooltip>
    </li>
  </ul>
</template>
