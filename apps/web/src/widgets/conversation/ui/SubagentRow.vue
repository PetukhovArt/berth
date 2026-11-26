<script setup lang="ts">
import { computed } from 'vue';
import { CheckCircle2, LoaderCircle, Square, TriangleAlert } from '../../../shared/ui/icons';
import { vTooltip } from '../../../shared/ui/kit';
import { UiButton } from '../../../shared/ui/UiButton';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { modelLabel, subagentSeconds, type Subagent } from '../../../entities/session';
import { durationLabel } from '../model/duration';

const props = defineProps<{ agent: Subagent; now: number; openable: boolean }>();
const emit = defineEmits<{ open: [callId: string] }>();

const { t } = useI18n();
const seconds = computed(() => subagentSeconds(props.agent, props.now));
const ICONS = { running: LoaderCircle, done: CheckCircle2, failed: TriangleAlert, stopped: Square } as const;
</script>

<template>
  <UiButton v-tooltip="openable ? t('chat.subagents.open', { name: agent.description }) : agent.description" type="button" class="row" :disabled="!openable" :data-state="agent.state" :data-call-id="agent.callId" data-testid="subagent-row" @click="emit('open', agent.callId)">
    <component :is="ICONS[agent.state]" :size="14" class="mark" />
    <span class="text">
      <span class="name" data-testid="subagent-row-name">{{ agent.description }}</span>
      <span class="meta">
        <span class="kind">{{ agent.kind ?? t('chat.subagents.noType') }}</span>
        <span v-if="agent.model" class="model">{{ modelLabel(agent.model) }}</span>
      </span>
    </span>
    <span class="when">
      <span v-if="seconds !== null" class="time" data-testid="subagent-row-time">{{ durationLabel(seconds) }}</span>
      <span class="state" data-testid="subagent-row-state">{{ t(`chat.subagents.state.${agent.state}`) }}</span>
    </span>
  </UiButton>
</template>

<style scoped>
.row {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  box-sizing: border-box;
  width: 100%;
  min-height: var(--vr-size-row);
  padding: var(--vr-space-1) var(--vr-space-3);
  font: inherit;
  color: var(--vr-color-text);
  text-align: left;
  cursor: pointer;
  background: none;
  border: 0;
}

:root[data-vr-viewport='phone'] .row {
  min-height: var(--vr-size-touch);
}

.row:hover:not(:disabled) {
  background: var(--vr-color-nav-hover);
}

.row:disabled {
  cursor: default;
}

.mark {
  flex: none;
  color: var(--vr-color-text-subtle);
}

.row[data-state='running'] .mark {
  color: var(--vr-status-running);
}

.row[data-state='done'] .mark {
  color: var(--vr-status-live);
}

.row[data-state='failed'] .mark {
  color: var(--vr-status-crashed);
}

.text {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
}

.name {
  overflow: hidden;
  font-size: var(--vr-font-size-label);
  white-space: nowrap;
  text-overflow: ellipsis;
}

.meta {
  display: flex;
  gap: var(--vr-space-2);
  min-width: 0;
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
  white-space: nowrap;
}

.kind,
.model {
  overflow: hidden;
  text-overflow: ellipsis;
}

.when {
  display: flex;
  flex: none;
  flex-direction: column;
  align-items: flex-end;
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
}

.time {
  font-variant-numeric: tabular-nums;
}
</style>
