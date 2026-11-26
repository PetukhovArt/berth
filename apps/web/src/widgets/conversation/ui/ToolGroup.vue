<script setup lang="ts">
import { computed, shallowRef } from 'vue';
import { ChevronDown, ChevronRight } from '../../../shared/ui/icons';
import { VRowButton, vTooltip } from '../../../shared/ui/kit';
import RichContent from '../../../shared/ui/rich-content/RichContent.vue';
import { t } from '../../../shared/lib/i18n/i18n';
import { withinWorktree } from '../../../shared/lib/worktreePath';
import { toolCounts, type ToolCall } from '../model/chatBlocks';
import { durationLabel } from '../model/duration';

const props = defineProps<{ calls: ToolCall[]; seconds: number | null; root: string | null }>();

const open = shallowRef(false);

const summary = computed(() => {
  const text = toolCounts(props.calls)
    .map(([category, count]) => t(`chat.tools.${category}`, { count }))
    .join(', ');
  return text.charAt(0).toUpperCase() + text.slice(1);
});
</script>

<template>
  <div class="group" :data-open="open" data-testid="chat-tool-group">
    <VRowButton v-tooltip="t('chat.tools.expand')" class="head" :aria-expanded="open" data-testid="chat-tool-group-toggle" @click="open = !open">
      <template #leading>
        <ChevronDown v-if="open" :size="10" />
        <ChevronRight v-else :size="10" />
      </template>
      <span data-testid="chat-tool-group-summary">{{ summary }}</span>
      <template v-if="seconds !== null" #trailing>
        <span class="time">{{ durationLabel(seconds) }}</span>
      </template>
    </VRowButton>
    <ol v-if="open" class="calls" data-testid="chat-tool-calls">
      <li v-for="call in calls" :key="call.message.key" class="call" :data-category="call.category" data-testid="chat-tool-call">
        <div class="row">
          <span class="name">{{ call.message.tool }}</span>
          <span v-tooltip="call.message.text" class="arg mono">{{ withinWorktree(call.message.text, root) }}</span>
          <span v-if="call.category === 'run' && call.message.exit !== undefined" class="exit mono" :data-failed="call.message.exit !== 0 || undefined" data-testid="chat-tool-exit">{{ t('chat.tools.exit', { code: call.message.exit }) }}</span>
        </div>
        <RichContent v-for="detail in call.details" :key="detail.key" class="detail" :markdown="detail.markdown ?? detail.text" />
      </li>
    </ol>
  </div>
</template>

<style scoped>
.group {
  overflow: hidden;
  background: var(--vr-color-surface);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-lg);
}

.head {
  gap: var(--vr-space-2);
  min-height: 36px;
  padding-block: 0;
  padding-inline: var(--vr-space-3);
  color: var(--vr-color-text-muted);
  border-radius: 0;
}

.time {
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-subtle);
}

.calls {
  margin: 0;
  padding: 0;
  list-style: none;
}

.call {
  border-top: 1px solid var(--vr-color-border);
}

.row {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
  min-height: 30px;
  padding: 0 var(--vr-space-3) 0 30px;
  font-size: var(--vr-font-size-label);
}

.name {
  flex: none;
  min-width: 40px;
  color: var(--vr-color-text-muted);
}

.arg {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text);
  white-space: nowrap;
  text-overflow: ellipsis;
}

.exit {
  flex: none;
  font-size: var(--vr-font-size-2xs);
  white-space: nowrap;
  color: var(--vr-color-text-subtle);
}

.exit[data-failed] {
  color: var(--vr-color-danger);
}

.detail {
  padding: 0 var(--vr-space-3) var(--vr-space-2) 30px;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.mono {
  font-family: var(--vr-font-mono);
}
</style>
