<script setup lang="ts">
import { computed, shallowRef } from 'vue';
import { ChevronDown, ChevronUp } from '../../../shared/ui/icons';
import { VRowButton, vTooltip } from '../../../shared/ui/kit';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { subagentCounts, type Subagent } from '../../../entities/session';
import SubagentRow from './SubagentRow.vue';

const props = defineProps<{ agents: readonly Subagent[]; now: number }>();
const emit = defineEmits<{ open: [callId: string] }>();

const { t } = useI18n();
const open = shallowRef(true);
const counts = computed(() => subagentCounts(props.agents));
const names = computed(() => props.agents.filter((agent) => agent.state === 'running').map((agent) => agent.description).join(', '));
</script>

<template>
  <div class="strip" :data-open="open" data-testid="subagent-strip">
    <VRowButton v-tooltip="open ? t('chat.subagents.collapse') : t('chat.subagents.expand')" class="head" :aria-expanded="open" data-testid="subagent-strip-toggle" @click="open = !open">
      <template #leading>
        <ChevronDown v-if="open" :size="12" />
        <ChevronUp v-else :size="12" />
      </template>
      <span class="body">
        <span class="title">{{ t('chat.subagents.strip') }}</span>
        <span class="summary" data-testid="subagent-strip-summary">
          <span v-if="counts.running" class="working" data-testid="subagent-strip-working">{{ t('chat.subagents.runningOf', { running: counts.running, total: agents.length }) }} · {{ names }}</span>
          <span v-if="counts.done">{{ t('chat.subagents.done', { count: counts.done }) }}</span>
          <span v-if="counts.failed" class="failed">{{ t('chat.subagents.failed', { count: counts.failed }) }}</span>
          <span v-if="counts.stopped">{{ t('chat.subagents.stopped', { count: counts.stopped }) }}</span>
        </span>
      </span>
    </VRowButton>
    <ul v-if="open" class="rows" data-testid="subagent-strip-rows">
      <li v-for="agent in agents" :key="agent.callId" class="item">
        <SubagentRow :agent="agent" :now="now" :openable="true" @open="emit('open', $event)" />
      </li>
    </ul>
  </div>
</template>

<style scoped>
.strip {
  flex: none;
  margin-bottom: var(--vr-space-2);
  overflow: hidden;
  background: var(--vr-color-surface);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-lg);
}

.head {
  gap: var(--vr-space-2);
  min-height: var(--vr-size-row);
  padding-block: 0;
  padding-inline: var(--vr-space-3);
  color: var(--vr-color-text);
  border-radius: 0;
}

:root[data-vr-viewport='phone'] .head {
  min-height: var(--vr-size-touch);
}

.body {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  min-width: 0;
}

.title {
  flex: none;
  font-weight: 600;
}

.summary {
  display: flex;
  flex: 1;
  flex-wrap: wrap;
  gap: 0 var(--vr-space-2);
  min-width: 0;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.summary > span + span::before {
  margin-right: var(--vr-space-2);
  content: '·';
}

.working {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.failed {
  color: var(--vr-color-danger);
}

.rows {
  max-height: min(40dvh, 20rem);
  margin: 0;
  padding: 0;
  overflow-y: auto;
  list-style: none;
  border-top: 1px solid var(--vr-color-border);
}

.item + .item {
  border-top: 1px solid var(--vr-color-border);
}
</style>
