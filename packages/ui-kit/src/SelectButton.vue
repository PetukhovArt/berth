<script setup lang="ts">
import SelectButton from 'primevue/selectbutton';
import { vTooltip } from './Tooltip';

interface SegmentOption {
  value: string;
  label: string;
  title?: string;
  testId?: string;
  disabled?: boolean;
}

const props = withDefaults(defineProps<{ options: SegmentOption[]; label: string; multiple?: boolean; allowEmpty?: boolean; size?: 'small' | 'large' }>(), { allowEmpty: undefined, size: 'small' });
const model = defineModel<string | string[] | null>();
const empty = () => props.allowEmpty ?? props.multiple;
</script>

<template>
  <SelectButton v-model="model" class="vr-segmented" :options="options" option-label="label" option-value="value" option-disabled="disabled" :multiple="multiple" :allow-empty="empty()" :size="size" :aria-label="label">
    <template #option="{ option }">
      <span v-tooltip="option.title" class="vr-segmented-option" :data-testid="option.testId">
        <slot name="option" :option="option">{{ option.label }}</slot>
      </span>
    </template>
  </SelectButton>
</template>
