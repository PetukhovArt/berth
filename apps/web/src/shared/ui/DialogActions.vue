<script setup lang="ts">
import { computed, useSlots } from 'vue';
import { VButton, useIsPhone } from './kit';
import { t } from '../lib/i18n/i18n';

const props = withDefaults(
  defineProps<{
    confirmLabel?: string;
    cancelLabel?: string;
    danger?: boolean;
    busy?: boolean;
    disabled?: boolean;
    submit?: boolean;
    focus?: 'primary' | 'cancel' | 'none';
    cancelTestid?: string;
    confirmTestid?: string;
  }>(),
  { focus: 'primary' },
);
const emit = defineEmits<{ cancel: []; confirm: [] }>();

const slots = useSlots();
const phone = useIsPhone();
const size = computed(() => (phone.value ? undefined : 'small'));

function isSingle(): boolean {
  return !props.confirmLabel && !slots.middle;
}
</script>

<template>
  <div class="dialog-actions">
    <div v-if="$slots.start" class="start"><slot name="start" :size="size" /></div>
    <VButton
      class="action"
      type="button"
      :label="cancelLabel ?? (isSingle() ? t('common.close') : t('common.cancel'))"
      :size="size"
      :text="!isSingle()"
      :severity="isSingle() ? undefined : 'secondary'"
      :autofocus="focus === 'cancel' || (isSingle() && focus === 'primary')"
      :data-testid="cancelTestid"
      @click="emit('cancel')"
    />
    <slot name="middle" :size="size" />
    <VButton
      v-if="confirmLabel"
      class="action"
      :type="submit ? 'submit' : 'button'"
      :label="confirmLabel"
      :size="size"
      :severity="danger ? 'danger' : undefined"
      :loading="busy"
      :disabled="disabled || busy"
      :autofocus="focus === 'primary'"
      :data-testid="confirmTestid"
      @click="emit('confirm')"
    />
  </div>
</template>

<style scoped>
.dialog-actions {
  display: flex;
  width: 100%;
  align-items: center;
  justify-content: flex-end;
  gap: var(--vr-space-2);
}

.start {
  display: flex;
  flex: 1;
  align-items: center;
  gap: var(--vr-space-3);
  min-width: 0;
  margin-right: auto;
}
</style>
