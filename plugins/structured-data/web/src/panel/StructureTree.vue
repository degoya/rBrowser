<script setup lang="ts">
// The structured data as Nuxt UI's tree (RB-090-96): a node per JSON-LD object or Microdata /
// RDFa item with its types, syntax and `@id`, its properties below it (a value on the same line,
// a nested node or a reference as a child), the worst finding at an entry as a badge with its
// word and count. The keyboard is the WAI-ARIA tree pattern's (reka-ui). Choosing a reference
// shows the node it points to; the other entries open and close.
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { SEVERITY_ICON, text, type TreeItem } from './model';

defineProps<{ items: TreeItem[] }>();
const expanded = defineModel<string[]>('expanded', { required: true });
const emit = defineEmits<{ show: [node: string] }>();
const { strings } = usePanel();

function onSelect(event: Event, item: TreeItem): void {
  if (!item.target) return;
  event.preventDefault();
  emit('show', item.target);
}
</script>

<template>
  <UTree
    id="tree"
    v-model:expanded="expanded"
    :items="items"
    :get-key="(item: TreeItem) => item.key"
    size="sm"
    color="neutral"
    :ui="{ linkLabel: 'flex min-w-0 flex-wrap items-center gap-1.5 whitespace-normal text-left' }"
    @select="(event: Event, item: TreeItem) => onSelect(event, item)"
  >
    <template #item-label="{ item }">
      <span
        :id="(item as TreeItem).node ? `node-${(item as TreeItem).node}` : undefined"
        :data-node="(item as TreeItem).node"
        data-label
        tabindex="-1"
        class="min-w-0 break-all outline-none"
        :class="(item as TreeItem).node ? 'font-medium text-highlighted' : 'text-default'"
        >{{ (item as TreeItem).label }}</span
      >
      <span v-if="(item as TreeItem).note" data-note class="text-xs text-muted">{{ (item as TreeItem).note }}</span>
      <UIcon v-if="(item as TreeItem).target" name="i-lucide-corner-down-right" class="size-3.5 shrink-0 text-muted" :aria-label="text(strings, 'panel.showRef')" />
      <UBadge
        v-if="(item as TreeItem).severity"
        data-severity
        :data-tone="(item as TreeItem).severity"
        :color="(item as TreeItem).severity"
        variant="subtle"
        size="sm"
        :icon="SEVERITY_ICON[(item as TreeItem).severity ?? 'info']"
        :label="`${text(strings, `panel.severity.${(item as TreeItem).severity}`)} ${(item as TreeItem).count ?? ''}`.trim()"
        :title="text(strings, 'panel.findingsAt', { count: (item as TreeItem).count ?? 0 })"
      />
    </template>
  </UTree>
</template>
