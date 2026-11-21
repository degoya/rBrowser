<script setup lang="ts">
// The raw JSON-LD blocks of the page (RB-090-96): each block with its element path, why it is no
// JSON when it is not, and its text as the page has it, coloured as JSON by panel-ui's code view.
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import CodeBlock from '../../../../web-common/panel-ui/src/code/CodeBlock.vue';
import { type Block, text } from './model';

defineProps<{ blocks: Block[] }>();
const { strings } = usePanel();
</script>

<template>
  <div class="flex flex-col gap-3">
    <p v-if="blocks.length === 0" id="no-blocks" class="text-sm text-muted">{{ text(strings, 'panel.noBlocks') }}</p>
    <section v-for="block in blocks" :id="`block-${block.index}`" :key="block.index" data-block class="flex flex-col gap-1.5">
      <h3 class="flex flex-wrap items-baseline gap-2 text-sm font-medium text-highlighted">
        <span>{{ text(strings, 'panel.block', { index: block.index + 1 }) }}</span>
        <span class="text-xs font-normal break-all text-muted">{{ block.path }}</span>
      </h3>
      <UAlert
        v-if="block.error"
        data-error
        color="error"
        variant="subtle"
        icon="i-lucide-circle-x"
        :title="text(strings, 'panel.blockError', { reason: block.error })"
        :ui="{ root: 'px-3 py-1.5', title: 'text-sm font-normal' }"
      />
      <CodeBlock v-if="block.text !== null" :text="block.text" :lang="block.error ? null : 'json'" />
      <p v-else data-dropped class="text-sm text-muted">{{ text(strings, 'panel.rawDropped') }}</p>
      <p v-if="block.textCut" data-cut class="text-xs text-muted">{{ text(strings, 'panel.rawCut') }}</p>
    </section>
  </div>
</template>
