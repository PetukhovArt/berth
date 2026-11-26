<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { UiButton } from '../../../shared/ui/UiButton';
import { FileText } from '../../../shared/ui/icons';
import { useSheet } from '../../../shared/lib/useSheet';
import { t } from '../../../shared/lib/i18n/i18n';
import { readAcpAgentLog } from '../../../shared/api/providers';
import { agentLogFor, toggleAgentLog } from '../../../shared/model/agentLog';
import { describeError } from '../../../entities/session';

const props = defineProps<{ agentId: string }>();

const open = computed({
  get: () => agentLogFor.value === props.agentId,
  set: (value) => {
    if (!value && agentLogFor.value === props.agentId) agentLogFor.value = null;
  },
});
const sheet = ref<HTMLElement | null>(null);
useSheet(sheet, open);
const text = ref<string | null>(null);

watch(
  open,
  async (value) => {
    if (!value) return;
    text.value = null;
    try {
      text.value = await readAcpAgentLog(props.agentId);
    } catch (failure) {
      text.value = describeError(failure);
    }
  },
  { immediate: true },
);
</script>

<template>
  <div class="agent-log">
    <UiButton type="button" class="chip" :title="t('agents.sessionView.chatAgentLog')" :aria-label="t('agents.sessionView.chatAgentLog')" :aria-expanded="open" data-testid="chat-agent-log-toggle" @click="toggleAgentLog(agentId)">
      <FileText :size="13" />
      <span class="chip-label">{{ t('agents.sessionView.chatAgentLog') }}</span>
    </UiButton>
    <div v-if="open" ref="sheet" class="sheet vr-sheet" data-testid="chat-agent-log">
      <span class="heading">{{ t('agents.sessionView.chatAgentLog') }}</span>
      <pre class="text mono" data-testid="chat-agent-log-text">{{ text === null ? '' : text || t('agents.sessionView.chatAgentLogEmpty') }}</pre>
    </div>
  </div>
</template>

<style scoped>
.agent-log {
  position: relative;
  display: flex;
  flex: none;
}

.chip {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  color: var(--vr-color-text-subtle);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-full);
  cursor: pointer;
}

.chip-label {
  display: none;
}

:root[data-vr-viewport='phone'] .chip {
  gap: var(--vr-space-1);
  width: auto;
  height: var(--vr-size-row-touch);
  padding: 0 var(--vr-space-3);
  font: inherit;
  font-size: var(--vr-font-size-label);
  color: var(--vr-color-text);
  background: var(--vr-color-surface-muted);
  border-radius: var(--vr-radius-md);
}

:root[data-vr-viewport='phone'] .chip-label {
  display: inline;
}

.sheet {
  position: absolute;
  top: 28px;
  right: 0;
  z-index: 6;
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  width: min(560px, 90vw);
  padding: var(--vr-space-3);
  background: var(--vr-color-surface-raised);
  border-radius: var(--vr-radius-md);
  box-shadow: var(--vr-shadow-popover);
}

.heading {
  font-size: var(--vr-font-size-sm);
  font-weight: 600;
}

.text {
  max-height: 320px;
  margin: 0;
  overflow: auto;
  font-size: var(--vr-font-size-2xs);
  white-space: pre-wrap;
  color: var(--vr-color-text-muted);
}
</style>
