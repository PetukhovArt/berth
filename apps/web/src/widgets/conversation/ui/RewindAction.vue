<script setup lang="ts">
import { shallowRef } from 'vue';
import { History } from '../../../shared/ui/icons';
import { VDialog } from '../../../shared/ui/kit';
import { UiButton } from '../../../shared/ui/UiButton';
import DialogActions from '../../../shared/ui/DialogActions.vue';
import { t } from '../../../shared/lib/i18n/i18n';
import { acpRewind, describeError } from '../../../entities/session';

const props = defineProps<{ agentId: string; seq: number; files: boolean; agentLabel: string }>();
const emit = defineEmits<{ rewound: [] }>();

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
    await acpRewind(props.agentId, props.seq);
    confirming.value = false;
    emit('rewound');
  } catch (failure) {
    error.value = describeError(failure);
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <div class="rewind">
    <UiButton type="button" class="link" data-testid="chat-rewind" @click="open">
      <History :size="11" />
      {{ t('chat.rewind') }}
    </UiButton>
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

.link {
  display: inline-flex;
  gap: var(--vr-space-1);
  align-items: center;
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
