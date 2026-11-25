<script setup lang="ts">
import { shallowRef } from 'vue';
import { RotateCcw } from '../../../shared/ui/icons';
import { VDialog } from '../../../shared/ui/kit';
import { UiButton } from '../../../shared/ui/UiButton';
import DialogActions from '../../../shared/ui/DialogActions.vue';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { describeError, restoreCheckpoint, type Checkpoint } from '../../../entities/session';

const props = defineProps<{ agentId: string; checkpoint: Checkpoint; place: string; agentLabel: string }>();
const emit = defineEmits<{ restored: [] }>();

const { t, formatTime } = useI18n();
const confirming = shallowRef(false);
const busy = shallowRef(false);
const error = shallowRef<string | null>(null);

function open() {
  error.value = null;
  confirming.value = true;
}

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
    <span class="label" :title="t('chat.checkpointTitle', { number: checkpoint.number })">
      <RotateCcw :size="11" />
      {{ t('chat.checkpoint') }}
      <span class="time">{{ formatTime(checkpoint.created_at * 1000) }}</span>
    </span>
    <span class="dot" aria-hidden="true">·</span>
    <UiButton type="button" class="link" data-testid="chat-checkpoint-restore" @click="open">{{ t('chat.restore') }}</UiButton>
    <VDialog v-model:visible="confirming" modal :header="t('chat.restoreTitle', { place: place || t('chat.checkpointWorktree') })" :style="{ width: '480px', maxWidth: '95vw' }" :draggable="false">
      <div class="confirm" data-testid="chat-checkpoint-confirm">
        <p class="text">{{ t('chat.restoreBody', { agent: agentLabel }) }}</p>
        <p v-if="error" class="error" role="alert">{{ error }}</p>
        <DialogActions :confirm-label="t('chat.restoreApply')" :busy="busy" focus="cancel" cancel-testid="chat-checkpoint-confirm-cancel" confirm-testid="chat-checkpoint-confirm-restore" @cancel="confirming = false" @confirm="restore" />
      </div>
    </VDialog>
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
  font-size: var(--vr-font-size-note);
  color: var(--vr-color-text-subtle);
}

.label {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-1);
}

.link {
  padding: 0;
  font: inherit;
  color: var(--vr-color-link);
  cursor: pointer;
  background: none;
  border: 0;
}

.confirm {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-3);
}

.text {
  margin: 0;
  color: var(--vr-color-text-muted);
}

.error {
  margin: 0;
  color: var(--vr-color-danger);
}
</style>
