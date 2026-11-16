<script setup lang="ts">
import { computed, shallowRef, watch } from 'vue';
import { RotateCcw } from '../../../shared/ui/icons';
import { UiButton } from '../../../shared/ui/UiButton';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { useCheckpoints, type AgentKind, type Checkpoint } from '../../../entities/session';
import InspectorSection from './InspectorSection.vue';

const props = defineProps<{ agentId: string; kind: AgentKind; refreshKey?: unknown }>();

const { t, formatDate } = useI18n();
const { items, busy, error, available, restore } = useCheckpoints(
  () => props.agentId,
  () => props.kind,
  () => props.refreshKey,
);
const pending = shallowRef<number | null>(null);
const newestFirst = computed(() => [...items.value].sort((a, b) => b.number - a.number));

function label(item: Checkpoint): string {
  return t(item.before_restore ? 'checkpoints.beforeRestore' : 'checkpoints.entry', { number: item.number });
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
      <li v-for="item in newestFirst" :key="item.number" :data-testid="`inspector-checkpoint-${item.number}`">
        <div v-if="pending === item.number" class="confirm" role="alertdialog" data-testid="inspector-checkpoint-confirm">
          <p class="note">{{ t('checkpoints.confirm', { number: item.number }) }}</p>
          <div class="actions">
            <UiButton type="button" class="secondary" @click="pending = null">{{ t('checkpoints.cancel') }}</UiButton>
            <UiButton type="button" class="primary" :disabled="busy" data-testid="inspector-checkpoint-confirm-restore" @click="confirm(item.number)">{{ t('checkpoints.confirmRestore') }}</UiButton>
          </div>
        </div>
        <UiButton
          v-else
          type="button"
          class="row entry"
          :title="t('checkpoints.restore')"
          :data-testid="`inspector-checkpoint-restore-${item.number}`"
          @click="pending = item.number"
        >
          <RotateCcw class="icon" :size="12" />
          <span class="grow">{{ label(item) }}</span>
          <span class="end">{{ formatDate(item.created_at * 1000, { timeStyle: 'short' }) }}</span>
        </UiButton>
      </li>
    </ul>
    <p v-if="error" class="error" role="alert" data-testid="inspector-checkpoints-error">{{ error }}</p>
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

.primary,
.secondary {
  height: 26px;
  padding: 0 10px;
  font: inherit;
  font-size: var(--vr-font-size-label);
  border: 0;
  border-radius: 7px;
  cursor: pointer;
}

.primary {
  color: var(--vr-color-accent-contrast);
  font-weight: 600;
  background: var(--vr-color-accent);
}

.secondary {
  color: var(--vr-color-text);
  background: var(--vr-color-surface-muted);
}
</style>
