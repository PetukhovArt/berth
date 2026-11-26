<script setup lang="ts">
import { computed, shallowRef } from 'vue';
import { Bot, ChevronDown, ChevronRight } from '../../../shared/ui/icons';
import { VRowButton, vTooltip } from '../../../shared/ui/kit';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { subagentCounts, subagentsSeconds, type Subagent, type SubagentState } from '../../../entities/session';
import { durationLabel } from '../model/duration';
import SubagentRow from './SubagentRow.vue';

const props = defineProps<{ agents: readonly Subagent[]; now: number; openable: boolean }>();
const emit = defineEmits<{ open: [callId: string] }>();

const { t } = useI18n();
const open = shallowRef(false);
const STATES: readonly SubagentState[] = ['running', 'done', 'failed', 'stopped'];
const LABELS: Record<SubagentState, (count: number) => string> = {
  running: (count) => t('chat.subagents.running', { count }),
  done: (count) => t('chat.subagents.done', { count }),
  failed: (count) => t('chat.subagents.failed', { count }),
  stopped: (count) => t('chat.subagents.stopped', { count }),
};
const chips = computed(() => {
  const counts = subagentCounts(props.agents);
  return STATES.filter((state) => counts[state] > 0).map((state) => ({ state, count: counts[state] }));
});
const seconds = computed(() => subagentsSeconds(props.agents, props.now));
</script>

<template>
  <div class="group" :data-open="open" data-testid="subagent-group">
    <VRowButton v-tooltip="t('chat.subagents.expand')" class="head" :aria-expanded="open" data-testid="subagent-group-toggle" @click="open = !open">
      <template #leading>
        <ChevronDown v-if="open" :size="10" />
        <ChevronRight v-else :size="10" />
      </template>
      <span class="body">
        <Bot :size="13" class="bot" />
        <span class="summary" data-testid="subagent-group-summary">{{ t('chat.subagents.group', { count: agents.length }) }}</span>
        <span class="chips">
          <span v-for="chip in chips" :key="chip.state" class="chip" :data-state="chip.state" data-testid="subagent-group-chip">
            <span class="dot" aria-hidden="true" />
            {{ LABELS[chip.state](chip.count) }}
          </span>
        </span>
      </span>
      <template v-if="seconds !== null" #trailing>
        <span class="time" data-testid="subagent-group-time">{{ durationLabel(seconds) }}</span>
      </template>
    </VRowButton>
    <ul v-if="open" class="rows" data-testid="subagent-group-rows">
      <li v-for="agent in agents" :key="agent.callId" class="item">
        <SubagentRow :agent="agent" :now="now" :openable="openable" @open="emit('open', $event)" />
      </li>
    </ul>
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
  padding-inline: var(--vr-space-3);
  color: var(--vr-color-text-muted);
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

.bot {
  flex: none;
}

.summary {
  flex: none;
}

.chips {
  display: flex;
  flex: 1;
  flex-wrap: wrap;
  gap: var(--vr-space-1) var(--vr-space-2);
  min-width: 0;
}

.chip {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-1);
  padding: 0 var(--vr-space-2);
  font-size: var(--vr-font-size-2xs);
  line-height: 1.8;
  white-space: nowrap;
  background: var(--vr-color-chip-bg);
  border-radius: var(--vr-radius-full);
}

.dot {
  width: 0.5em;
  height: 0.5em;
  background: var(--vr-color-text-subtle);
  border-radius: 50%;
}

.chip[data-state='running'] .dot {
  background: var(--vr-status-running);
}

.chip[data-state='done'] .dot {
  background: var(--vr-status-live);
}

.chip[data-state='failed'] .dot {
  background: var(--vr-status-crashed);
}

.time {
  font-size: var(--vr-font-size-xs);
  white-space: nowrap;
}

.rows {
  margin: 0;
  padding: 0;
  list-style: none;
}

.item {
  border-top: 1px solid var(--vr-color-border);
}
</style>
