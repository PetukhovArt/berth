<script setup lang="ts">
import { nextTick, shallowRef, useTemplateRef } from 'vue';
import { UiButton } from '../../../shared/ui/UiButton';
import UiTextarea from '../../../shared/ui/UiTextarea.vue';
import { t } from '../../../shared/lib/i18n/i18n';
import type { AcpQueuedPrompt } from '../../../entities/session';
import StoredAttachments from './StoredAttachments.vue';

const props = defineProps<{ item: AcpQueuedPrompt; agentLabel: string }>();
const emit = defineEmits<{ edit: [id: number, text: string]; remove: [id: number] }>();

const editing = shallowRef(false);
const draft = shallowRef('');
const input = useTemplateRef<InstanceType<typeof UiTextarea>>('input');

async function startEdit() {
  draft.value = props.item.prompt.text;
  editing.value = true;
  await nextTick();
  input.value?.focus();
}

function save() {
  const text = draft.value.trim();
  if (text && text !== props.item.prompt.text) emit('edit', props.item.id, text);
  editing.value = false;
}

function onKey(event: KeyboardEvent) {
  if (event.isComposing) return;
  if (event.key === 'Escape') {
    event.stopPropagation();
    editing.value = false;
  } else if (event.key === 'Enter' && !event.shiftKey) {
    event.preventDefault();
    save();
  }
}
</script>

<template>
  <div class="queued" data-testid="queue-item">
    <StoredAttachments v-if="item.prompt.attachments.length" :attachments="item.prompt.attachments" />
    <div class="bubble">
      <UiTextarea v-if="editing" ref="input" v-model="draft" class="edit" rows="2" :aria-label="t('chat.queued.editLabel')" data-testid="queue-item-input" @keydown="onKey" />
      <p v-else class="text" data-testid="queue-item-text">{{ item.prompt.text }}</p>
    </div>
    <p class="hint">{{ t('chat.queued.text', { agent: agentLabel }) }}</p>
    <div class="actions">
      <template v-if="editing">
        <UiButton type="button" class="link" data-testid="queue-save" @click="save">{{ t('chat.queued.save') }}</UiButton>
        <UiButton type="button" class="link" data-testid="queue-cancel" @click="editing = false">{{ t('chat.queued.cancel') }}</UiButton>
      </template>
      <UiButton v-else type="button" class="link" data-testid="queue-edit" @click="startEdit">{{ t('chat.queued.edit') }}</UiButton>
      <UiButton type="button" class="link" data-testid="queue-remove" @click="emit('remove', item.id)">{{ t('chat.queued.remove') }}</UiButton>
    </div>
  </div>
</template>

<style scoped>
.queued {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-1);
  align-items: flex-end;
  max-width: 100%;
}

.bubble {
  box-sizing: border-box;
  min-width: 0;
  max-width: 100%;
  padding: 9px 14px;
  border: 1px dashed var(--vr-color-border-strong);
  border-radius: var(--vr-radius-bubble) var(--vr-radius-bubble) var(--vr-radius-chip) var(--vr-radius-bubble);
}

.text {
  margin: 0;
  font-size: var(--vr-font-size-sm);
  line-height: 1.57;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}

.edit {
  display: block;
  width: 100%;
  min-width: 240px;
  padding: 0;
  font: inherit;
  font-size: var(--vr-font-size-sm);
  line-height: 1.57;
  color: var(--vr-color-text);
  resize: none;
  background: none;
  border: 0;
  outline: none;
  field-sizing: content;
}

.hint {
  margin: 0;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-subtle);
}

.actions {
  display: flex;
  gap: var(--vr-space-3);
}

.link {
  padding: 0;
  font: inherit;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-link);
  cursor: pointer;
  background: none;
  border: 0;
}
</style>
