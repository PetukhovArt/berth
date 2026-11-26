<script setup lang="ts">
import { shallowRef } from 'vue';
import { RotateCcw } from '../../../shared/ui/icons';
import { VButton, vTooltip } from '../../../shared/ui/kit';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { describeError, restoreCheckpoint, type Checkpoint } from '../../../entities/session';

const props = defineProps<{ agentId: string; checkpoint: Checkpoint }>();
const emit = defineEmits<{ restored: [] }>();

const { t, formatTime } = useI18n();
const confirming = shallowRef(false);
const busy = shallowRef(false);
const error = shallowRef<string | null>(null);

async function restore() {
  if (busy.value) return;
  busy.value = true;
  error.value = null;
  try {
    await restoreCheckpoint(props.agentId, props.checkpoint.number);
    confirming.value = false;
    emit('restored');
  } catch (failure) {
    error.value = describeError(failure);
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <div class="checkpoint" :data-testid="`chat-checkpoint-${checkpoint.number}`">
    <div v-if="confirming" class="confirm" role="alertdialog" data-testid="chat-checkpoint-confirm">
      <span class="text">{{ t('chat.restoreConfirm') }}</span>
      <VButton :label="t('chat.restoreCancel')" text severity="secondary" size="small" @click="confirming = false" />
      <VButton :label="t('chat.restoreApply')" text size="small" :disabled="busy" data-testid="chat-checkpoint-confirm-restore" @click="restore" />
    </div>
    <template v-else>
      <span v-tooltip="t('chat.checkpointTitle', { number: checkpoint.number })" class="label">
        <RotateCcw :size="12" />
        {{ t('chat.checkpoint') }}
        <span class="time">{{ formatTime(checkpoint.created_at * 1000) }}</span>
      </span>
      <span class="dot" aria-hidden="true">·</span>
      <VButton :label="t('chat.restore')" text size="small" data-testid="chat-checkpoint-restore" @click="confirming = true" />
    </template>
    <span v-if="error" class="error" role="alert">{{ error }}</span>
  </div>
</template>

<style scoped>
.checkpoint {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: flex-end;
  gap: var(--vr-space-1) var(--vr-space-2);
  margin-top: var(--vr-space-1);
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.label {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-1);
}

.time,
.dot {
  color: var(--vr-color-text-subtle);
}

.confirm {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: flex-end;
  gap: var(--vr-space-2);
}

.text {
  flex: 1 1 100%;
  text-align: right;
}

.error {
  flex: 1 1 100%;
  color: var(--vr-color-danger);
  text-align: right;
}
</style>
