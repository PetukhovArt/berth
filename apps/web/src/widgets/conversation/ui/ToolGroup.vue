<script setup lang="ts">
import { computed, shallowRef } from 'vue';
import { ChevronDown, ChevronRight } from '../../../shared/ui/icons';
import { UiButton } from '../../../shared/ui/UiButton';
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
    <UiButton type="button" class="head" :aria-expanded="open" :title="t('chat.tools.expand')" data-testid="chat-tool-group-toggle" @click="open = !open">
      <ChevronDown v-if="open" :size="10" />
      <ChevronRight v-else :size="10" />
      <span class="summary" data-testid="chat-tool-group-summary">{{ summary }}</span>
      <span v-if="seconds !== null" class="time">{{ durationLabel(seconds) }}</span>
    </UiButton>
    <ol v-if="open" class="calls" data-testid="chat-tool-calls">
      <li v-for="call in calls" :key="call.message.key" class="call" :data-category="call.category" data-testid="chat-tool-call">
        <div class="row">
          <span class="name">{{ call.message.tool }}</span>
          <span class="arg mono" :title="call.message.text">{{ withinWorktree(call.message.text, root) }}</span>
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
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  width: 100%;
  min-height: 36px;
  padding: 0 var(--vr-space-3);
  font: inherit;
  font-size: var(--vr-font-size-label);
  color: var(--vr-color-text-muted);
  text-align: left;
  cursor: pointer;
  background: none;
  border: 0;
}

.head:hover {
  background: var(--vr-color-nav-hover);
}

.summary {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.time {
  flex: none;
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
