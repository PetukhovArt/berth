<script setup lang="ts">
import { onMounted, useTemplateRef } from 'vue';
import { ChevronDown, ChevronUp, X } from '../../../shared/ui/icons';
import { VButton, VInputText, vTooltip } from '../../../shared/ui/kit';
import { t } from '../../../shared/lib/i18n/i18n';

defineProps<{ index: number; total: number }>();
const query = defineModel<string>({ required: true });
const emit = defineEmits<{ step: [delta: number]; close: [] }>();

const input = useTemplateRef<{ $el: HTMLInputElement }>('input');

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
  input.value?.$el.focus();
});

defineExpose({ focus: () => input.value?.$el.focus() });
</script>

<template>
  <div class="find" role="search" data-testid="chat-find">
    <VInputText ref="input" v-model="query" class="field" type="search" :placeholder="t('chat.find.placeholder')" :aria-label="t('chat.find.placeholder')" data-testid="chat-find-input" @keydown="onKey" />
    <span class="count" role="status" data-testid="chat-find-count">{{ !query ? '' : total ? t('chat.find.count', { index: index + 1, total }) : t('chat.find.none') }}</span>
    <VButton v-tooltip="t('chat.find.previous')" class="step" text severity="secondary" size="small" :disabled="!total" :aria-label="t('chat.find.previous')" data-testid="chat-find-prev" @click="emit('step', -1)">
      <template #icon><ChevronUp :size="14" /></template>
    </VButton>
    <VButton v-tooltip="t('chat.find.next')" class="step" text severity="secondary" size="small" :disabled="!total" :aria-label="t('chat.find.next')" data-testid="chat-find-next" @click="emit('step', 1)">
      <template #icon><ChevronDown :size="14" /></template>
    </VButton>
    <VButton v-tooltip="t('chat.find.close')" class="step" text severity="secondary" size="small" :aria-label="t('chat.find.close')" data-testid="chat-find-close" @click="emit('close')">
      <template #icon><X :size="14" /></template>
    </VButton>
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
  border-radius: 0;
  box-shadow: none;
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
  flex: none;
  width: 26px;
  height: 26px;
  padding: 0;
  border-radius: var(--vr-radius-control);
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
