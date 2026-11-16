<script setup lang="ts">
// The mock editor (RB-070-02, on Nuxt UI since RB-080-84): URL pattern, method, the action as a
// choice of two (mock or block), status and delay for a mock, its body in syntax colours by what
// it looks like; a refused field or a failed save said in words. Save adds the rule (`mock_add`),
// Cancel or Esc closes.
import { computed, nextTick, onMounted } from 'vue';
import CodeEditor from '../../../../web-common/panel-ui/src/code/CodeEditor.vue';
import { highlighter } from '../../../../web-common/panel-ui/src/code/highlight';
import PanelTabs from '../../../../web-common/panel-ui/src/components/PanelTabs.vue';
import { useNetwork } from './store';

const net = useNetwork();
const mocks = net.mocks;
const editor = mocks.editor;
const t = net.panel.t;

const actions = computed(() => [
  { label: t('kinds.mock'), value: 'mock' },
  { label: t('kinds.block'), value: 'block' },
]);
const bodyLang = (text: string) => highlighter().detect(text);

onMounted(() => void nextTick(() => document.getElementById('pattern')?.focus()));
</script>

<template>
  <UCard id="editor" as="section" aria-labelledby="editor-title" :ui="{ header: 'px-3 py-2 sm:px-3', body: 'flex flex-col gap-2 p-3 sm:p-3' }" @keydown.esc="editor.open = false">
    <template #header>
      <h2 id="editor-title" class="text-sm font-semibold text-highlighted">{{ t('editor.title') }}</h2>
    </template>
    <div class="flex flex-wrap items-end gap-2">
      <UFormField :label="t('editor.pattern')" class="min-w-48 flex-1">
        <UInput id="pattern" v-model="editor.pattern" size="sm" :maxlength="2048" :spellcheck="false" class="w-full" />
      </UFormField>
      <UFormField :label="t('editor.method')">
        <UInput id="method" v-model="editor.method" size="sm" :maxlength="16" :spellcheck="false" class="w-24" />
      </UFormField>
    </div>
    <UFormField :label="t('editor.action')">
      <PanelTabs id="action" v-model="editor.action" :items="actions" :content="false" size="xs" class="w-auto" />
    </UFormField>
    <div v-if="editor.action === 'mock'" class="flex flex-wrap items-end gap-2">
      <UFormField :label="t('editor.status')">
        <UInput id="code" v-model="editor.status" type="number" :min="100" :max="599" size="sm" class="w-28" />
      </UFormField>
      <UFormField :label="t('editor.delay')">
        <UInput id="delay" v-model="editor.delay" type="number" :min="0" :max="60000" size="sm" class="w-28" />
      </UFormField>
    </div>
    <UFormField v-if="editor.action === 'mock'" :label="t('editor.body')">
      <CodeEditor id="mock-body" v-model="editor.body" :lang="bodyLang" :note="t('detail.plainLarge')" :rows="3" :label="t('editor.body')" />
    </UFormField>
    <UAlert v-if="editor.error" id="editor-error" role="alert" color="error" variant="subtle" icon="i-lucide-circle-x" :title="editor.error" :ui="{ root: 'px-3 py-1.5', title: 'text-sm font-normal' }" />
    <div class="flex gap-1.5">
      <UButton id="save" size="sm" icon="i-lucide-check" :label="t('editor.save')" @click="mocks.save()" />
      <UButton id="cancel" size="sm" variant="outline" color="neutral" :label="t('editor.cancel')" @click="editor.open = false" />
    </div>
  </UCard>
</template>
