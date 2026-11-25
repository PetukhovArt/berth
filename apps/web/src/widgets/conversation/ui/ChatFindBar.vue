<script setup lang="ts">
import { onMounted, useTemplateRef } from 'vue';
import { ChevronDown, ChevronUp, X } from '../../../shared/ui/icons';
import { UiButton } from '../../../shared/ui/UiButton';
import UiInput from '../../../shared/ui/UiInput.vue';
import { t } from '../../../shared/lib/i18n/i18n';

defineProps<{ index: number; total: number }>();
const query = defineModel<string>({ required: true });
const emit = defineEmits<{ step: [delta: number]; close: [] }>();

const input = useTemplateRef<InstanceType<typeof UiInput>>('input');

function onKey(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    event.stopPropagation();
    emit('close');
  } else if (event.key === 'Enter') {
    event.preventDefault();
    emit('step', event.shiftKey ? -1 : 1);
  }
}

onMounted(() => {
  input.value?.focus();
});

defineExpose({ focus: () => input.value?.focus() });
</script>

<template>
  <div class="find" role="search" data-testid="chat-find">
    <UiInput ref="input" v-model="query" class="field" type="search" :placeholder="t('chat.find.placeholder')" :aria-label="t('chat.find.placeholder')" data-testid="chat-find-input" @keydown="onKey" />
    <span class="count" role="status" data-testid="chat-find-count">{{ !query ? '' : total ? t('chat.find.count', { index: index + 1, total }) : t('chat.find.none') }}</span>
    <UiButton type="button" class="step" :disabled="!total" :title="t('chat.find.previous')" :aria-label="t('chat.find.previous')" data-testid="chat-find-prev" @click="emit('step', -1)"><ChevronUp :size="14" /></UiButton>
    <UiButton type="button" class="step" :disabled="!total" :title="t('chat.find.next')" :aria-label="t('chat.find.next')" data-testid="chat-find-next" @click="emit('step', 1)"><ChevronDown :size="14" /></UiButton>
    <UiButton type="button" class="step" :title="t('chat.find.close')" :aria-label="t('chat.find.close')" data-testid="chat-find-close" @click="emit('close')"><X :size="14" /></UiButton>
  </div>
</template>

<style scoped>
.find {
  position: absolute;
  top: var(--vr-space-2);
  right: var(--vr-space-6);
  z-index: 3;
  display: flex;
  align-items: center;
  gap: var(--vr-space-1);
  padding: var(--vr-space-1) var(--vr-space-1) var(--vr-space-1) var(--vr-space-3);
  background: var(--vr-color-surface-raised);
  border: 1px solid var(--vr-color-border-strong);
  border-radius: var(--vr-radius-xl);
  box-shadow: var(--vr-shadow-popover);
}

.field {
  width: 14em;
  min-width: 0;
  padding: 0;
  font: inherit;
  font-size: var(--vr-font-size-control);
  color: var(--vr-color-text);
  background: none;
  border: 0;
  outline: none;
}

.count {
  min-width: 4.5em;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-subtle);
  text-align: right;
  white-space: nowrap;
}

.step {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  padding: 0;
  color: var(--vr-color-text-muted);
  cursor: pointer;
  background: none;
  border: 0;
  border-radius: var(--vr-radius-control);
}

.step:hover:not(:disabled) {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}

.step:disabled {
  opacity: 0.4;
  cursor: default;
}
</style>

<style>
::highlight(chat-find) {
  color: var(--vr-color-text);
  background-color: var(--vr-color-match-bg);
}

::highlight(chat-find-current) {
  color: var(--vr-color-accent-contrast);
  background-color: var(--vr-color-accent);
}
</style>
