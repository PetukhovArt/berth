<script setup lang="ts">
import { nextTick, useTemplateRef } from 'vue';
import { VButton, VPopover } from './kit';
import { t } from '../lib/i18n/i18n';

defineProps<{ question: string; confirmLabel?: string; busy?: boolean; testid?: string }>();
const emit = defineEmits<{ confirm: []; cancel: [] }>();

const popover = useTemplateRef<InstanceType<typeof VPopover>>('popover');
const cancelButton = useTemplateRef<{ $el: HTMLElement }>('cancelButton');

function show(target: HTMLElement | null) {
  if (target) popover.value?.show(new MouseEvent('click'), target);
}

function hide() {
  popover.value?.hide();
}

async function focusCancel() {
  await nextTick();
  cancelButton.value?.$el.focus();
}

defineExpose({ show, hide });
</script>

<template>
  <VPopover ref="popover" @show="focusCancel" @hide="emit('cancel')">
    <div class="confirm" role="alertdialog" :aria-label="question" :data-testid="testid">
      <span class="question">{{ question }}</span>
      <div class="buttons">
        <VButton ref="cancelButton" :label="t('common.cancel')" size="small" text severity="secondary" data-testid="confirm-cancel" @click="hide" />
        <VButton :label="confirmLabel ?? t('common.delete')" size="small" severity="danger" :loading="busy" data-testid="confirm-remove" @click="emit('confirm')" />
      </div>
    </div>
  </VPopover>
</template>

<style scoped>
.confirm {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  max-width: 260px;
}

.question {
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text);
  overflow-wrap: anywhere;
}

.buttons {
  display: flex;
  justify-content: flex-end;
  gap: var(--vr-space-1);
}
</style>
