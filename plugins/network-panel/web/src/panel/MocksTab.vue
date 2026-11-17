<script setup lang="ts">
// The Mocks tab (RB-070-02, on Nuxt UI since RB-080-84): the mock editor when open (New rule,
// or Mock this request from the Requests tab), the mock files - each a card, blocked ones say so,
// their problems in words - with a switch per rule, its kind, pattern, method, status, hits and
// Remove; the block list of the settings; the HAR import.
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import MockEditor from './MockEditor.vue';
import { useNetwork } from './store';

const net = useNetwork();
const mocks = net.mocks;
const t = net.panel.t;
</script>

<template>
  <div id="panel-mocks" class="flex flex-col gap-3">
    <MockEditor v-if="mocks.editor.open" />

    <section aria-labelledby="mocks-title" class="flex flex-col gap-2">
      <div class="flex flex-wrap items-center gap-2">
        <h2 id="mocks-title" class="flex-1 text-sm font-semibold text-highlighted">{{ t('mocks.title') }}</h2>
      </div>
      <UAlert
        v-if="mocks.note.value"
        id="mocks-note"
        :color="mocks.answer.value && !mocks.answer.value.trusted ? 'warning' : 'neutral'"
        variant="subtle"
        icon="i-lucide-info"
        :title="mocks.note.value"
        :ui="{ root: 'px-3 py-1.5', title: 'text-sm font-normal' }"
      />
      <ul id="mocks" class="flex flex-col gap-2">
        <li v-for="file in mocks.answer.value?.files ?? []" :key="file.path" data-file class="rounded-md border border-default">
          <div class="flex flex-wrap items-center gap-2 border-b border-default px-2 py-1.5">
            <UIcon name="i-lucide-file-json" class="size-4 shrink-0 text-muted" />
            <span data-path class="min-w-0 flex-1 truncate font-mono text-xs" :class="file.blocked ? 'text-muted' : 'text-highlighted'" :title="file.path">{{ file.path }}</span>
            <UBadge v-if="file.blocked" data-blocked color="warning" variant="subtle" size="sm" icon="i-lucide-lock" :label="t('mocks.blocked')" />
          </div>
          <p v-for="(problem, index) in file.problems ?? []" :key="index" class="px-2 py-1 text-xs text-error">{{ problem }}</p>
          <ul class="divide-y divide-default">
            <li v-for="rule in file.rules" :key="rule.id" data-rule class="flex flex-wrap items-center gap-2 px-2 py-1.5">
              <UCheckbox size="sm" :model-value="rule.enabled" :aria-label="t('mocks.enableRule', { id: rule.id })" :data-rule="rule.id" @update:model-value="(on: boolean | 'indeterminate') => mocks.enable(rule.id, on === true)" />
              <UBadge color="neutral" variant="outline" size="sm" :label="t('kinds.' + rule.kind)" />
              <code class="min-w-0 flex-1 font-mono text-xs break-all text-highlighted">{{ rule.urlPattern }}</code>
              <UBadge v-if="rule.method" color="neutral" variant="subtle" size="sm" :label="rule.method" />
              <UBadge v-if="rule.kind === 'mock'" color="neutral" variant="subtle" size="sm" :label="String(rule.status ?? '')" />
              <span data-hits class="text-xs text-muted">{{ t('mocks.hits', { count: rule.hits }) }}</span>
              <span v-if="rule.rejected" class="text-xs text-error">{{ t('mocks.rejected', { reason: rule.rejected }) }}</span>
              <IconAction icon="i-lucide-trash-2" :label="t('mocks.removeRule', { id: rule.id })" :data-remove="rule.id" @click="mocks.remove(rule.id)" />
            </li>
          </ul>
        </li>
        <li v-if="(mocks.answer.value?.block ?? []).length" data-block class="rounded-md border border-default">
          <div class="flex items-center gap-2 border-b border-default px-2 py-1.5">
            <UIcon name="i-lucide-ban" class="size-4 shrink-0 text-muted" />
            <span class="flex-1 text-sm text-highlighted">{{ t('mocks.blockList') }}</span>
          </div>
          <ul class="divide-y divide-default">
            <li v-for="blocked in mocks.answer.value?.block ?? []" :key="blocked.id" class="flex flex-wrap items-center gap-2 px-2 py-1.5">
              <code class="min-w-0 flex-1 font-mono text-xs break-all text-highlighted">{{ blocked.urlPattern }}</code>
              <span data-hits class="text-xs text-muted">{{ t('mocks.hits', { count: blocked.hits }) }}</span>
            </li>
          </ul>
        </li>
      </ul>
      <UEmpty
        v-if="mocks.answer.value && !mocks.answer.value.files.length && !mocks.answer.value.block.length"
        id="mocks-empty"
        icon="i-lucide-flask-conical"
        :title="t('mocks.none')"
        variant="naked"
      />
      <div class="flex flex-wrap items-end gap-2">
        <UFormField :label="t('mocks.harPath')" class="min-w-48 flex-1">
          <UInput id="har-path" v-model="mocks.harPath.value" size="sm" :maxlength="512" :spellcheck="false" placeholder=".rbrowser/..." class="w-full" />
        </UFormField>
        <UButton id="har-import" size="sm" icon="i-lucide-file-input" variant="outline" color="neutral" :label="t('mocks.harImport')" @click="mocks.importHar()" />
      </div>
    </section>
  </div>
</template>
