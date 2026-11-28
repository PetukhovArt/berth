<script setup lang="ts">
import { computed, ref, useTemplateRef, watch } from 'vue';
import { History } from '../../../shared/ui/icons';
import { VButton, VMessage, VPopover, useIsPhone, vTooltip } from '../../../shared/ui/kit';
import { t, useI18n } from '../../../shared/lib/i18n/i18n';
import { toggleAt, type MenuAction } from '../../../shared/lib/menuAction';
import { useCheckpoints, type AgentKind, type Checkpoint } from '../../../entities/session';

const props = defineProps<{ agentId: string; name: string; kind: AgentKind; refreshKey?: unknown }>();

const { formatDate } = useI18n();
const { items, busy, error, available: visible, load, restore: restoreAt } = useCheckpoints(
  () => props.agentId,
  () => props.kind,
  () => props.refreshKey,
);
const open = ref(false);
const phone = useIsPhone();
const popover = useTemplateRef<InstanceType<typeof VPopover>>('popover');
const pending = ref<number | null>(null);

function toggle(event: Event) {
  pending.value = null;
  error.value = null;
  popover.value?.toggle(event);
}

function opened() {
  open.value = true;
  void load();
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

function openAt(target: HTMLElement) {
  pending.value = null;
  error.value = null;
  toggleAt(popover.value, target);
}

const item = computed<MenuAction | null>(() => (visible.value ? { label: t('checkpoints.open'), testid: 'session-menu-checkpoints', run: openAt } : null));
defineExpose({ item });

watch(
  () => props.agentId,
  () => popover.value?.hide(),
);
</script>

<template>
  <div v-if="visible" class="checkpoints">
    <VButton
      v-tooltip="t('checkpoints.open')"
      text
      severity="secondary"
      size="small"
      :label="phone ? t('checkpoints.open') : undefined"
      :aria-label="t('checkpoints.open')"
      :aria-expanded="open"
      aria-haspopup="dialog"
      data-testid="checkpoints-button"
      @click="toggle"
    >
      <template #icon><History :size="13" /></template>
    </VButton>
    <VPopover ref="popover" @show="opened" @hide="open = false">
      <div class="panel" data-testid="checkpoints-panel">
        <span class="heading">{{ t('checkpoints.title', { name }) }}</span>
        <span class="hint">{{ t('checkpoints.hint') }}</span>
        <span v-if="!items.length" class="hint" data-testid="checkpoints-empty">{{ t('checkpoints.empty') }}</span>
        <ul v-else class="list">
          <li v-for="item in items" :key="item.number" class="row" :data-testid="`checkpoint-${item.number}`">
            <div v-if="pending === item.number" class="confirm" role="alertdialog" data-testid="checkpoint-confirm">
              <span class="hint">{{ t('checkpoints.confirm', { number: item.number }) }}</span>
              <div class="actions">
                <VButton :label="t('checkpoints.cancel')" text severity="secondary" size="small" @click="pending = null" />
                <VButton :label="t('checkpoints.confirmRestore')" size="small" :disabled="busy" data-testid="checkpoint-confirm-restore" @click="restore(item.number)" />
              </div>
            </div>
            <template v-else>
              <span class="label">{{ label(item) }}</span>
              <span class="time">{{ time(item) }}</span>
              <VButton :label="t('checkpoints.restore')" link size="small" :data-testid="`checkpoint-restore-${item.number}`" @click="pending = item.number" />
            </template>
          </li>
        </ul>
        <VMessage v-if="error" severity="error" size="small" data-testid="checkpoints-error">{{ error }}</VMessage>
      </div>
    </VPopover>
  </div>
</template>

<style scoped>
.checkpoints {
  display: flex;
  flex: none;
}

.panel {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  width: min(340px, 80vw);
  max-height: 360px;
  overflow-y: auto;
}

.heading {
  font-size: var(--vr-font-size-sm);
  font-weight: 600;
}

.hint {
  color: var(--vr-color-text-subtle);
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
</style>
