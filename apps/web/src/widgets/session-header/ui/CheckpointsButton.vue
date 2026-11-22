<script setup lang="ts">
import { ref, watch } from 'vue';
import { UiButton } from '../../../shared/ui/UiButton';
import { History } from '../../../shared/ui/icons';
import { useSheetClamp } from '../../../shared/lib/useSheetClamp';
import { t, useI18n } from '../../../shared/lib/i18n/i18n';
import { useCheckpoints, type AgentKind, type Checkpoint } from '../../../entities/session';

const props = defineProps<{ agentId: string; name: string; kind: AgentKind; refreshKey?: unknown }>();

const { formatDate } = useI18n();
const { items, busy, error, available: visible, load, restore: restoreAt } = useCheckpoints(
  () => props.agentId,
  () => props.kind,
  () => props.refreshKey,
);
const open = ref(false);
const sheet = ref<HTMLElement | null>(null);
useSheetClamp(sheet, open);
const pending = ref<number | null>(null);

function toggle() {
  open.value = !open.value;
  pending.value = null;
  error.value = null;
  if (open.value) void load();
}

async function restore(number: number) {
  if (await restoreAt(number)) pending.value = null;
}

function label(item: Checkpoint): string {
  return t(item.before_restore ? 'checkpoints.beforeRestore' : 'checkpoints.entry', { number: item.number });
}

function time(item: Checkpoint): string {
  return formatDate(item.created_at * 1000, { timeStyle: 'short' });
}

watch(
  () => props.agentId,
  () => {
    open.value = false;
  },
);
</script>

<template>
  <div v-if="visible" class="checkpoints">
    <UiButton type="button" class="chip" :title="t('checkpoints.open')" :aria-label="t('checkpoints.open')" :aria-expanded="open" data-testid="checkpoints-button" @click="toggle">
      <History :size="13" />
      <span class="chip-label">{{ t('checkpoints.open') }}</span>
    </UiButton>
    <div v-if="open" ref="sheet" class="panel vr-sheet" data-testid="checkpoints-panel">
      <span class="heading">{{ t('checkpoints.title', { name }) }}</span>
      <span class="hint">{{ t('checkpoints.hint') }}</span>
      <span v-if="!items.length" class="hint" data-testid="checkpoints-empty">{{ t('checkpoints.empty') }}</span>
      <ul v-else class="list">
        <li v-for="item in items" :key="item.number" class="row" :data-testid="`checkpoint-${item.number}`">
          <div v-if="pending === item.number" class="confirm" role="alertdialog" data-testid="checkpoint-confirm">
            <span class="hint">{{ t('checkpoints.confirm', { number: item.number }) }}</span>
            <div class="actions">
              <UiButton type="button" class="secondary" @click="pending = null">{{ t('checkpoints.cancel') }}</UiButton>
              <UiButton type="button" class="primary" :disabled="busy" data-testid="checkpoint-confirm-restore" @click="restore(item.number)">{{ t('checkpoints.confirmRestore') }}</UiButton>
            </div>
          </div>
          <template v-else>
            <span class="label">{{ label(item) }}</span>
            <span class="time">{{ time(item) }}</span>
            <UiButton type="button" class="link" :data-testid="`checkpoint-restore-${item.number}`" @click="pending = item.number">{{ t('checkpoints.restore') }}</UiButton>
          </template>
        </li>
      </ul>
      <span v-if="error" class="error" role="alert" data-testid="checkpoints-error">{{ error }}</span>
    </div>
  </div>
</template>

<style scoped>
.checkpoints {
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

.panel {
  position: absolute;
  top: 28px;
  right: 0;
  z-index: 6;
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  width: 340px;
  max-height: 360px;
  padding: var(--vr-space-3);
  overflow-y: auto;
  background: var(--vr-color-surface-raised);
  border-radius: var(--vr-radius-md);
  box-shadow: var(--vr-shadow-popover);
}

.heading {
  font-size: var(--vr-font-size-sm);
  font-weight: 600;
}

.hint {
  color: var(--vr-color-text-subtle);
  font-size: var(--vr-font-size-xs);
}

.error {
  color: var(--vr-status-crashed);
  font-size: var(--vr-font-size-xs);
}

.list {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-1);
  padding: 0;
  margin: 0;
  list-style: none;
}

.row {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  font-size: var(--vr-font-size-sm);
}

.label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.time {
  color: var(--vr-color-text-subtle);
  font-size: var(--vr-font-size-xs);
}

.link {
  padding: 0;
  color: var(--vr-color-link);
  font: inherit;
  font-size: var(--vr-font-size-xs);
  background: none;
  border: 0;
  cursor: pointer;
}

.confirm {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: var(--vr-space-2);
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--vr-space-2);
}

.primary,
.secondary {
  height: 28px;
  padding: 0 11px;
  font: inherit;
  font-size: var(--vr-font-size-label);
  border: 0;
  border-radius: var(--vr-radius-sm);
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
