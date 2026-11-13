<script setup lang="ts">
import { shallowRef } from 'vue';
import { enqueuePrompt, removeQueuedPrompt, submitPrompt, useAgentBusy, useQueuedPrompts } from '../../../entities/session';
import { t } from '../../../shared/lib/i18n/i18n';
import { UiButton } from '../../../shared/ui/UiButton';
import UiInput from '../../../shared/ui/UiInput.vue';

const props = defineProps<{ agent: string }>();
const queued = useQueuedPrompts(props.agent);
const busy = useAgentBusy(props.agent);
const draft = shallowRef('');

function submit() {
  const text = draft.value.trim();
  if (!text) return;
  draft.value = '';
  if (busy.value) enqueuePrompt(props.agent, text);
  else void submitPrompt({ Agent: props.agent }, text);
}

function edit(id: number, text: string) {
  removeQueuedPrompt(props.agent, id);
  draft.value = text;
}
</script>

<template>
  <div class="queue" data-testid="queue-strip">
    <div v-for="item in queued" :key="item.id" class="item" data-testid="queue-item">
      <span class="text" data-testid="queue-item-text">{{ item.text }}</span>
      <span class="hint">{{ t('feedback.queue.sendsWhenFinished') }}</span>
      <UiButton type="button" class="link" data-testid="queue-edit" @click="edit(item.id, item.text)">{{ t('feedback.queue.edit') }}</UiButton>
      <UiButton type="button" class="link" data-testid="queue-remove" @click="removeQueuedPrompt(agent, item.id)">{{ t('feedback.queue.remove') }}</UiButton>
    </div>
    <UiInput :model-value="draft" class="field" data-testid="queue-input" :placeholder="busy ? t('feedback.queue.placeholderBusy') : t('feedback.queue.placeholder')" @update:model-value="draft = String($event)" @keydown.enter.exact.prevent="submit" />
  </div>
</template>

<style scoped>
.queue {
  display: flex;
  flex: none;
  flex-direction: column;
  gap: 6px;
  padding: 7px 12px 7px 16px;
  border-top: 1px solid var(--vr-color-border);
}

.item {
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: 24px;
  padding: 0 10px;
  border: 1px dashed var(--vr-color-border-strong);
  border-radius: var(--vr-radius-md);
}

.text {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  font-size: var(--vr-font-size-sm);
  color: var(--vr-color-text);
  white-space: nowrap;
  text-overflow: ellipsis;
}

.hint {
  flex: none;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.link {
  flex: none;
  padding: 0;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-link);
  cursor: pointer;
  background: none;
  border: 0;
}

.field {
  box-sizing: border-box;
  height: 28px;
  padding: 0 10px;
  font-size: var(--vr-font-size-sm);
  color: var(--vr-color-text);
  background: var(--vr-color-surface-muted);
  border: 0;
  border-radius: var(--vr-radius-md);
  outline: none;
}
</style>
