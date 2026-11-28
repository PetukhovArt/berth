<script setup lang="ts">
import { computed, nextTick, shallowRef, useTemplateRef, watch } from 'vue';
import { RotateCcw } from '../../../shared/ui/icons';
import { VButton, VMessage, VRowButton, vTooltip } from '../../../shared/ui/kit';
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

const list = useTemplateRef<HTMLElement>('list');

watch(
  () => props.agentId,
  () => {
    pending.value = null;
  },
);

watch(pending, async (number) => {
  if (number === null) return;
  await nextTick();
  list.value?.querySelector('[data-testid="inspector-checkpoint-confirm"]')?.scrollIntoView({ block: 'nearest' });
});
</script>

<template>
  <InspectorSection v-if="available" :title="t('inspector.checkpoints.title')" :meta="items.length ? String(items.length) : undefined" testid="inspector-checkpoints">
    <p v-if="!items.length" class="note" data-testid="inspector-checkpoints-empty">{{ t('checkpoints.empty') }}</p>
    <ul v-else ref="list" class="list">
      <li v-for="item in chronological" :key="item.number" :data-testid="`inspector-checkpoint-${item.number}`">
        <div v-if="pending === item.number" class="confirm" role="alertdialog" data-testid="inspector-checkpoint-confirm">
          <p class="note">{{ t('checkpoints.confirm', { number: item.number }) }}</p>
          <div class="actions">
            <VButton :label="t('checkpoints.cancel')" text severity="secondary" size="small" @click="pending = null" />
            <VButton :label="t('checkpoints.confirmRestore')" size="small" :disabled="busy" data-testid="inspector-checkpoint-confirm-restore" @click="confirm(item.number)" />
          </div>
        </div>
        <VRowButton v-else v-tooltip="`${label(item)}\n${t('checkpoints.restore')}`" class="entry" :data-testid="`inspector-checkpoint-restore-${item.number}`" @click="pending = item.number">
          <template #leading><RotateCcw class="icon" :size="12" /></template>
          {{ label(item) }}
          <template #trailing>
            <span class="end">{{ formatDate(item.created_at * 1000, { timeStyle: 'short' }) }}</span>
          </template>
        </VRowButton>
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
  gap: var(--vr-space-2);
  min-height: 22px;
  padding-block: 0;
  padding-inline: 6px;
  font-size: var(--vr-font-size-sm);
  color: var(--vr-color-text);
  border-radius: var(--vr-radius-sm);
}

.icon {
  color: var(--vr-color-text-subtle);
}

.end {
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-subtle);
  white-space: nowrap;
  font-variant-numeric: tabular-nums;
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
