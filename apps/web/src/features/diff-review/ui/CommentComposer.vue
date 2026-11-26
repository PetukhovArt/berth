<script setup lang="ts">
import { computed, onMounted, useTemplateRef } from 'vue';
import { VButton, VMessage, VTextarea } from '../../../shared/ui/kit';
import { t } from '../../../shared/lib/i18n/i18n';

const props = defineProps<{ testid: string; busy: boolean; error: string | null; placeholder: string; label: string; cancellable?: boolean; autofocus?: boolean }>();
const emit = defineEmits<{ send: []; cancel: [] }>();
const draft = defineModel<string>({ required: true });

const input = useTemplateRef<{ $el: HTMLTextAreaElement }>('input');
const canSend = computed(() => !props.busy && draft.value.trim().length > 0);

function onKey(event: KeyboardEvent) {
  if (event.isComposing) return;
  if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) {
    event.preventDefault();
    if (canSend.value) emit('send');
  } else if (event.key === 'Escape') {
    event.preventDefault();
    event.stopPropagation();
    emit('cancel');
  }
}

onMounted(() => {
  if (props.autofocus) input.value?.$el.focus();
});
</script>

<template>
  <form class="composer" @submit.prevent="canSend && emit('send')">
    <VTextarea
      ref="input"
      v-model="draft"
      class="input"
      rows="3"
      :readonly="busy"
      :placeholder="placeholder"
      :aria-label="placeholder"
      :data-testid="`${testid}-input`"
      @keydown="onKey"
    />
    <VMessage v-if="error" severity="error" size="small" :data-testid="`${testid}-error`">{{ error }}</VMessage>
    <div class="row">
      <VButton type="submit" size="small" :label="label" :loading="busy" :disabled="!canSend" :data-testid="`${testid}-send`" />
      <VButton v-if="cancellable" type="button" size="small" severity="secondary" text :label="t('common.cancel')" :disabled="busy" @click="emit('cancel')" />
      <span class="hint">{{ t('forge.composer.hint') }}</span>
    </div>
  </form>
</template>

<style scoped>
.composer {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
}

.input {
  width: 100%;
  min-height: 4.5rem;
  resize: vertical;
}

.row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-2);
}

.hint {
  margin-left: auto;
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
}
</style>
