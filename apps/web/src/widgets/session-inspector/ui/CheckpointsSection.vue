<script setup lang="ts">
import { computed, shallowRef, watch } from 'vue';
import { RotateCcw } from '../../../shared/ui/icons';
import { VButton, VMessage, vTooltip } from '../../../shared/ui/kit';
import { UiButton } from '../../../shared/ui/UiButton';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { promptOfCheckpoint, useAgentMessages, useCheckpoints, type AgentKind, type Checkpoint } from '../../../entities/session';
import InspectorSection from './InspectorSection.vue';

const PROMPT_TAIL = 600;

const props = defineProps<{ agentId: string; kind: AgentKind; backend?: 'chat' | 'terminal'; sessionId?: string | null; refreshKey?: unknown }>();

const { t, formatDate } = useI18n();
const { items, busy, error, available, restore } = useCheckpoints(
  () => props.agentId,
  () => props.kind,
  () => props.refreshKey,
);
const messages = useAgentMessages(
  () => ({ id: props.agentId, kind: props.kind, backend: props.backend, session_id: props.sessionId ?? null }),
  PROMPT_TAIL,
  () => [props.refreshKey, items.value.length],
);
const pending = shallowRef<number | null>(null);
const chronological = computed(() => [...items.value].sort((a, b) => a.number - b.number));
const prompts = computed(() => promptOfCheckpoint(messages.value, items.value));

function label(item: Checkpoint): string {
  if (item.before_restore) return t('checkpoints.beforeRestore', { number: item.number });
  return prompts.value.get(item.number) ?? t('checkpoints.entry', { number: item.number });
}

async function confirm(number: number) {
  if (await restore(number)) pending.value = null;
}

watch(
  () => props.agentId,
  () => {
    pending.value = null;
  },
);
</script>

<template>
  <InspectorSection v-if="available" :title="t('inspector.checkpoints.title')" :meta="items.length ? String(items.length) : undefined" testid="inspector-checkpoints">
    <p v-if="!items.length" class="note" data-testid="inspector-checkpoints-empty">{{ t('checkpoints.empty') }}</p>
    <ul v-else class="list">
      <li v-for="item in chronological" :key="item.number" :data-testid="`inspector-checkpoint-${item.number}`">
        <div v-if="pending === item.number" class="confirm" role="alertdialog" data-testid="inspector-checkpoint-confirm">
          <p class="note">{{ t('checkpoints.confirm', { number: item.number }) }}</p>
          <div class="actions">
            <VButton :label="t('checkpoints.cancel')" text severity="secondary" size="small" @click="pending = null" />
            <VButton :label="t('checkpoints.confirmRestore')" size="small" :disabled="busy" data-testid="inspector-checkpoint-confirm-restore" @click="confirm(item.number)" />
          </div>
        </div>
        <UiButton
          v-else
          v-tooltip="`${label(item)}\n${t('checkpoints.restore')}`"
          type="button"
          class="row entry"
          :data-testid="`inspector-checkpoint-restore-${item.number}`"
          @click="pending = item.number"
        >
          <RotateCcw class="icon" :size="12" />
          <span class="grow">{{ label(item) }}</span>
          <span class="end">{{ formatDate(item.created_at * 1000, { timeStyle: 'short' }) }}</span>
        </UiButton>
      </li>
    </ul>
    <VMessage v-if="error" severity="error" size="small" data-testid="inspector-checkpoints-error">{{ error }}</VMessage>
  </InspectorSection>
</template>

<style scoped>
.list {
  display: flex;
  flex-direction: column;
  gap: 2px;
  max-height: 220px;
  padding: 0;
  margin: 0 -6px;
  overflow-y: auto;
  list-style: none;
}

.entry {
  width: 100%;
  padding: 0 6px;
  font: inherit;
  text-align: left;
  background: none;
  border: 0;
  border-radius: var(--vr-radius-sm);
  cursor: pointer;
}

.entry:hover,
.entry:focus-visible {
  background: var(--vr-color-nav-hover);
}

.confirm {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  padding: var(--vr-space-2) 6px;
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--vr-space-2);
}
</style>
