<script setup lang="ts">
import { shallowRef } from 'vue';
import { History } from '../../../shared/ui/icons';
import { VButton, VDialog } from '../../../shared/ui/kit';
import DialogActions from '../../../shared/ui/DialogActions.vue';
import { t } from '../../../shared/lib/i18n/i18n';
import { describeError } from '../../../entities/session';

const props = defineProps<{ seq: number; files: boolean; agentLabel: string; apply: (seq: number) => Promise<void> }>();

const confirming = shallowRef(false);
const busy = shallowRef(false);
const error = shallowRef<string | null>(null);

function open() {
  error.value = null;
  confirming.value = true;
}

async function rewind() {
  if (busy.value) return;
  busy.value = true;
  error.value = null;
  try {
    await props.apply(props.seq);
    confirming.value = false;
  } catch (failure) {
    error.value = describeError(failure);
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <div class="rewind">
    <VButton :label="t('chat.rewind')" link size="small" data-testid="chat-rewind" @click="open">
      <template #icon><History :size="11" /></template>
    </VButton>
    <VDialog v-model:visible="confirming" modal :header="t('chat.rewindTitle')" :style="{ width: '480px', maxWidth: '95vw' }" :draggable="false">
      <div class="confirm" data-testid="chat-rewind-confirm">
        <p class="text">{{ t(files ? 'chat.rewindBodyFiles' : 'chat.rewindBody', { agent: agentLabel }) }}</p>
        <p v-if="error" class="error" role="alert">{{ error }}</p>
        <DialogActions :confirm-label="t('chat.rewindApply')" :busy="busy" danger focus="cancel" cancel-testid="chat-rewind-cancel" confirm-testid="chat-rewind-apply" @cancel="confirming = false" @confirm="rewind" />
      </div>
    </VDialog>
  </div>
</template>

<style scoped>
.rewind {
  display: flex;
  justify-content: flex-end;
  font-size: var(--vr-font-size-note);
  color: var(--vr-color-text-subtle);
  opacity: 0;
  transition: opacity var(--vr-motion-slow) var(--vr-motion-ease-standard);
}

.message:hover .rewind,
.rewind:focus-within {
  opacity: 1;
}

@media (hover: none) {
  .rewind {
    opacity: 1;
  }
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
